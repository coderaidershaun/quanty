//! The question being asked, the three results that come back one after the other, and the
//! rules that keep an old reply from landing under a new question.

use std::collections::BTreeSet;

use export::{Block, Document, Format, Source, fence_for};

use super::HealthLevel;
use super::shared::{Shared, push_cancel};
use contract::{
    Answer, AnswerBlock, AskDraft, AskMode, AskView, Command, ConceptGraph, ConceptId, Effect,
    ExportPlace, ExportRequest, Failure, Filters, ItemKind, Loadable, RequestId, ResultItem,
    SearchReply, Tab,
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

    // SMELL: this file is over 400 lines. The answer written out for an export, from here down to
    // `source_line`, is the part to move out first.

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

    /// The written answer as a document that can be put in a file. `None` exactly when
    /// `answer_markdown` is `None`.
    pub fn answer_document(&self) -> Option<Document> {
        let answer = self.answer.ready()?;
        if answer.blocks.is_empty() {
            return None;
        }
        let mut blocks = Vec::new();
        let mut cited = BTreeSet::new();
        for block in &answer.blocks {
            match block {
                AnswerBlock::Heading(heading) => blocks.push(Block::Heading(heading.clone())),
                AnswerBlock::Paragraph { text, cites } => {
                    blocks.push(Block::Text {
                        markdown: dollar_math(text),
                        cites: cites.clone(),
                    });
                    cited.extend(cites);
                }
                AnswerBlock::Item(number) => {
                    if let Some(result) = self.result(*number) {
                        blocks.extend(card_block(result));
                        cited.insert(number);
                    }
                }
            }
        }
        let sources = cited
            .into_iter()
            .filter_map(|number| self.result(*number))
            .map(|result| Source {
                number: result.number,
                text: source_place(result),
            })
            .collect();
        Some(Document {
            title: answer
                .title
                .clone()
                .unwrap_or_else(|| self.question.clone()),
            origin: format!("Question: {}", self.question),
            blocks,
            sources,
        })
    }

    /// Prepares the answer for export, with its download name and the place that asked.
    /// Returns `None` when there is no written answer.
    pub fn export_request(&self, format: Format) -> Option<ExportRequest> {
        Some(ExportRequest {
            format,
            document: self.answer_document()?,
            file_name: format.file_name(&self.question),
            place: ExportPlace::Answer {
                generation: self.generation,
            },
        })
    }
}

/// The block that draws the card of a result. A passage has no card.
fn card_block(result: &ResultItem) -> Option<Block> {
    let cite = Some(result.number);
    match result.kind {
        ItemKind::Formula => Some(Block::Formula {
            latex: result.text.clone(),
            label: result.label.clone(),
            caption: "As printed in the source".to_owned(),
            cite,
        }),
        ItemKind::Figure => Some(Block::Figure {
            picture: result
                .image
                .as_ref()
                .map(|image| std::path::PathBuf::from(&image.url)),
            description: result.text.clone(),
            caption: result.caption(),
            cite,
        }),
        ItemKind::Table => Some(Block::Table {
            markdown: result.text.clone(),
            caption: result.caption(),
            cite,
        }),
        ItemKind::Code => Some(Block::Code {
            code: result.text.clone(),
            language: result.language.clone(),
            caption: result.caption(),
            cite,
        }),
        ItemKind::Chunk => None,
    }
}

/// The text of a paragraph for a reader of Markdown with dollar-sign formulas: `\( … \)` becomes
/// `$…$`, a code span and a backslash escape stay as they are, and any other `$` is escaped so
/// that an amount of money is not read as the start of a formula.
fn dollar_math(text: &str) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut converted = String::with_capacity(text.len());
    let mut at = 0;
    while at < characters.len() {
        match characters[at] {
            '\\' => {
                if characters.get(at + 1) == Some(&'(')
                    && let Some(end) = (at + 2..characters.len().saturating_sub(1))
                        .find(|&i| characters[i] == '\\' && characters[i + 1] == ')')
                {
                    let latex: String = characters[at + 2..end].iter().collect();
                    let latex = latex.trim();
                    if !latex.is_empty() {
                        converted.push('$');
                        converted.push_str(latex);
                        converted.push('$');
                        at = end + 2;
                        continue;
                    }
                }
                converted.push('\\');
                if let Some(&escaped) = characters.get(at + 1) {
                    converted.push(escaped);
                    at += 2;
                } else {
                    at += 1;
                }
            }
            '`' => {
                let end = (at + 2..characters.len()).find(|&i| characters[i] == '`');
                let stop = end.map_or(at + 1, |end| end + 1);
                converted.extend(&characters[at..stop]);
                at = stop;
            }
            '$' => {
                converted.push_str("\\$");
                at += 1;
            }
            other => {
                converted.push(other);
                at += 1;
            }
        }
    }
    converted
}

fn card_markdown(result: &ResultItem) -> String {
    let number = result.number;
    let name = match result.kind {
        ItemKind::Formula => return format!("$$ {} $$ [{number}]", result.text),
        ItemKind::Code => {
            let title = result.label.as_deref().unwrap_or("Code");
            let language = result.language.as_deref().unwrap_or_default();
            let fence = fence_for(&result.text);
            return format!(
                "*{title}* [{number}]\n\n{fence}{language}\n{}\n{fence}",
                result.text
            );
        }
        ItemKind::Figure => "Figure",
        ItemKind::Table => "Table",
        ItemKind::Chunk => "Text",
    };
    format!("*{}* [{number}]", result.label.as_deref().unwrap_or(name))
}

/// The document and the page as printed, with the label of the item when it has one.
fn source_place(result: &ResultItem) -> String {
    let pages = match (
        &result.printed_page,
        result.last_page,
        &result.last_printed_page,
    ) {
        (Some(first), Some(_), Some(last)) => format!("pp. {first}\u{2013}{last}"),
        (_, Some(last_page), _) => format!("pp. positions {}\u{2013}{last_page}", result.page),
        (Some(printed), None, _) => format!("p. {printed}"),
        (None, None, _) => format!("p. position {}", result.page),
    };
    let mut place = format!("{}, {pages}", result.doc_title);
    if let Some(label) = &result.label {
        place.push_str(&format!(" \u{2014} {label}"));
    }
    place
}

fn source_line(result: &ResultItem) -> String {
    format!("[{}] {}", result.number, source_place(result))
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
        self.ask_view = AskView::Answer;
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
        self.source_from_library = false;
        self.tab = Tab::Ask;
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
        self.ask.graph = self.settled(result);
    }

    pub(super) fn answer_arrived(&mut self, request: RequestId, result: Result<Answer, Failure>) {
        if self.ask.request != Some(request) || !self.ask.answer.is_loading() {
            return;
        }
        self.ask.answer = self.settled(result);
    }
}
