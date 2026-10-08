//! The question being asked, the three results that come back one after the other, and the
//! rules that keep an old reply from landing under a new question.

use std::collections::BTreeSet;

use super::HealthLevel;
use super::shared::{Shared, push_cancel};
use crate::contract::{
    Answer, AnswerBlock, AskDraft, AskMode, Command, ConceptGraph, ConceptId, Effect, Failure,
    Filters, ItemKind, Loadable, RequestId, ResultItem, SearchReply, Tab,
};

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct AskSession {
    /// Goes up by one on every new ask. Panels reset their own state when it changes.
    pub generation: u64,
    pub request: Option<RequestId>,
    /// The question as it was submitted.
    pub question: String,
    pub mode: AskMode,
    pub filters: Filters,
    pub search: Loadable<SearchReply>,
    pub graph: Loadable<ConceptGraph>,
    /// Stays `Idle` in mode `ResultsOnly`.
    pub answer: Loadable<Answer>,
    /// The number of the selected result.
    pub selected_result: Option<usize>,
    pub focused_concept: Option<ConceptId>,
}

impl AskSession {
    pub fn is_running(&self) -> bool {
        self.search.is_loading() || self.graph.is_loading() || self.answer.is_loading()
    }

    /// Empty until the search is ready.
    pub fn results(&self) -> &[ResultItem] {
        match &self.search {
            Loadable::Ready(reply) => &reply.results,
            _ => &[],
        }
    }

    pub fn result(&self, number: usize) -> Option<&ResultItem> {
        self.results().iter().find(|result| result.number == number)
    }

    /// `None` while there is no answer or it has no block.
    pub fn answer_markdown(&self) -> Option<String> {
        let answer = self.answer.ready()?;
        if answer.blocks.is_empty() {
            return None;
        }
        let title = answer.title.as_deref().unwrap_or(&self.question);
        let mut parts = vec![format!("# {title}")];
        let mut cited = BTreeSet::new();
        for block in &answer.blocks {
            match block {
                AnswerBlock::Heading(heading) => parts.push(format!("## {heading}")),
                AnswerBlock::Paragraph { text, cites } => {
                    let marks: String = cites.iter().map(|number| format!(" [{number}]")).collect();
                    parts.push(format!("{text}{marks}"));
                    cited.extend(cites);
                }
                AnswerBlock::Item(number) => {
                    if let Some(result) = self.result(*number) {
                        parts.push(card_markdown(result));
                        cited.insert(number);
                    }
                }
            }
        }
        let sources: Vec<String> = cited
            .into_iter()
            .filter_map(|number| self.result(*number))
            .map(source_line)
            .collect();
        let mut markdown = parts.join("\n\n");
        if !sources.is_empty() {
            markdown.push_str("\n\nSources\n");
            markdown.push_str(&sources.join("\n"));
        }
        Some(markdown)
    }
}

fn card_markdown(result: &ResultItem) -> String {
    let number = result.number;
    let name = match result.kind {
        ItemKind::Formula => return format!("$$ {} $$ [{number}]", result.text),
        ItemKind::Figure => "Figure",
        ItemKind::Table => "Table",
        ItemKind::Chunk => "Text",
    };
    format!("*{}* [{number}]", result.label.as_deref().unwrap_or(name))
}

fn source_line(result: &ResultItem) -> String {
    let page = result
        .printed_page
        .clone()
        .unwrap_or_else(|| format!("position {}", result.page));
    let mut line = format!("[{}] {}, p. {page}", result.number, result.doc_title);
    if let Some(label) = &result.label {
        line.push_str(&format!(" \u{2014} {label}"));
    }
    line
}

fn stop_waiting<T>(slot: &mut Loadable<T>) {
    if slot.is_loading() {
        *slot = Loadable::Idle;
    }
}

impl Shared {
    pub(super) fn start_ask(&mut self, draft: AskDraft, effects: &mut Vec<Effect>) {
        if draft.question.trim().is_empty() {
            return;
        }
        let request = self.issue_request();
        if let (true, Some(old)) = (self.ask.is_running(), self.ask.request) {
            push_cancel(effects, old);
        }
        self.ask = AskSession {
            generation: self.ask.generation + 1,
            request: Some(request),
            question: draft.question.clone(),
            mode: draft.mode,
            filters: draft.filters.clone(),
            search: Loadable::Loading,
            graph: Loadable::Loading,
            answer: match draft.mode {
                AskMode::Answer => Loadable::Loading,
                AskMode::ResultsOnly => Loadable::Idle,
            },
            selected_result: None,
            focused_concept: None,
        };
        self.tab = Tab::Ask;
        effects.push(Effect::Send(Command::Ask {
            request,
            ask: draft,
        }));
    }

    pub(super) fn cancel_ask(&mut self, effects: &mut Vec<Effect>) {
        if !self.ask.is_running() {
            return;
        }
        stop_waiting(&mut self.ask.search);
        stop_waiting(&mut self.ask.graph);
        stop_waiting(&mut self.ask.answer);
        if let Some(request) = self.ask.request {
            push_cancel(effects, request);
        }
    }

    pub(super) fn select_result(&mut self, number: usize, effects: &mut Vec<Effect>) {
        let Some(result) = self.ask.result(number) else {
            return;
        };
        let (doc, page, piece) = (result.doc, result.page, result.piece);
        self.ask.selected_result = Some(number);
        self.open_source(doc, page, piece, effects);
    }

    pub(super) fn step_result(&mut self, delta: i32, effects: &mut Vec<Effect>) {
        let results = self.ask.results();
        if results.is_empty() {
            return;
        }
        let current = self
            .ask
            .selected_result
            .and_then(|number| results.iter().position(|result| result.number == number));
        let step = isize::try_from(delta).unwrap_or_default();
        let index = current.map_or(0, |at| {
            at.saturating_add_signed(step).min(results.len() - 1)
        });
        if let Some(number) = results.get(index).map(|result| result.number) {
            self.select_result(number, effects);
        }
    }

    pub(super) fn search_arrived(
        &mut self,
        request: RequestId,
        result: Result<SearchReply, Failure>,
        effects: &mut Vec<Effect>,
    ) {
        if self.ask.request != Some(request) || !self.ask.search.is_loading() {
            return;
        }
        match result {
            Ok(reply) => {
                let found_nothing = reply.results.is_empty();
                let was_down = self.health.level() == HealthLevel::Down;
                self.ask.search = Loadable::Ready(reply);
                if found_nothing {
                    self.ask.graph = Loadable::Ready(ConceptGraph::default());
                    self.ask.answer = Loadable::Idle;
                }
                if was_down {
                    self.recheck_health(effects);
                }
            }
            Err(failure) => {
                self.mark_down(&failure);
                self.ask.search = Loadable::Failed(failure);
                self.ask.graph = Loadable::Idle;
                self.ask.answer = Loadable::Idle;
            }
        }
    }

    pub(super) fn graph_arrived(
        &mut self,
        request: RequestId,
        result: Result<ConceptGraph, Failure>,
    ) {
        if self.ask.request != Some(request) || !self.ask.graph.is_loading() {
            return;
        }
        if let Err(failure) = &result {
            self.mark_down(failure);
        }
        self.ask.graph = result.into();
    }

    pub(super) fn answer_arrived(&mut self, request: RequestId, result: Result<Answer, Failure>) {
        if self.ask.request != Some(request) || !self.ask.answer.is_loading() {
            return;
        }
        if let Err(failure) = &result {
            self.mark_down(failure);
        }
        self.ask.answer = result.into();
    }
}
