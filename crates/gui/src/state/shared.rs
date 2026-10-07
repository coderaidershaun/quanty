//! The state of the whole window, and the one place that matches each intent and each event to
//! the rule that handles it.

use std::path::PathBuf;

use super::{AskSession, Health, IngestJob, Library, SourceNav};
use crate::contract::{
    Command, Effect, Event, Intent, ItemCounts, Notice, NoticeId, NoticeKind, RequestId,
    StartupFacts, Tab,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Quit {
    #[default]
    No,
    Asking,
    Confirmed,
}

/// Counters that only go up. A panel acts when one is above what it saw last.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Cues {
    /// The Ask bar focuses its box.
    pub focus_ask_bar: u64,
    /// The Ingest panel copies `picked_pdf` into its draft.
    pub pdf_picks: u64,
    pub picked_pdf: Option<PathBuf>,
    /// The source view shows its target again.
    pub source_shows: u64,
}

/// What every panel reads. `Default` is for tests; the app makes it with `new`.
#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct Shared {
    pub tab: Tab,
    pub help_open: bool,
    pub quit: Quit,
    pub cues: Cues,
    pub ask: AskSession,
    pub source: SourceNav,
    pub library: Library,
    pub ingest: IngestJob,
    /// Newest first, this session only.
    pub notices: Vec<Notice>,
    pub health: Health,
    next_request: u64,
    next_notice: u64,
}

impl Shared {
    pub fn new(facts: StartupFacts) -> Shared {
        Shared {
            health: Health::new(facts),
            ..Shared::default()
        }
    }

    /// Applies what a person asked for. Whatever the app must then do is pushed to `effects`.
    pub fn apply_intent(&mut self, intent: Intent, effects: &mut Vec<Effect>) {
        match intent {
            Intent::OpenTab(tab) => self.tab = tab,
            Intent::ToggleHelp => self.help_open = !self.help_open,
            Intent::FocusAskBar => {
                self.tab = Tab::Ask;
                self.cues.focus_ask_bar += 1;
            }
            Intent::CopyText(text) => effects.push(Effect::CopyText(text)),
            Intent::ShareAnswer => {
                if let Some(markdown) = self.ask.answer_markdown() {
                    effects.push(Effect::CopyText(markdown));
                }
            }
            Intent::Ask(draft) => self.start_ask(draft, effects),
            Intent::CancelAsk => self.cancel_ask(effects),
            Intent::SelectResult(number) => self.select_result(number, effects),
            Intent::StepResult(delta) => self.step_result(delta, effects),
            Intent::FocusConcept(concept) => self.ask.focused_concept = concept,
            Intent::OpenSource { doc, page, piece } => self.open_source(doc, page, piece, effects),
            Intent::TurnPage(delta) => self.turn_page(delta, effects),
            Intent::ReloadSource => self.reload_source(effects),
            Intent::RefreshCatalogue => self.refresh_catalogue(effects),
            Intent::SetLabels(edit) => self.set_labels(edit, effects),
            Intent::DeleteDocument(doc) => self.delete_document(doc, effects),
            Intent::PickPdf => effects.push(Effect::PickFile),
            Intent::PdfPicked(path) => {
                self.cues.picked_pdf = Some(path);
                self.cues.pdf_picks += 1;
            }
            Intent::CheckIngest(request) => self.check_ingest(request, effects),
            Intent::StartIngest => self.start_ingest(effects),
            Intent::CancelIngest => self.cancel_ingest(effects),
            Intent::ClearIngest => self.clear_ingest(effects),
            Intent::DismissNotice(id) => self.notices.retain(|notice| notice.id != id),
            Intent::MarkNoticesRead => {
                self.notices
                    .iter_mut()
                    .for_each(|notice| notice.read = true);
            }
            Intent::RecheckHealth => self.recheck_health(effects),
            Intent::RequestQuit => self.request_quit(effects),
            Intent::ConfirmQuit => self.confirm_quit(effects),
            Intent::DismissQuit => self.quit = Quit::No,
        }
    }

    /// Applies what the backend sent. An event whose request is not the one its slot waits for
    /// changes nothing.
    pub fn apply_event(&mut self, event: Event, effects: &mut Vec<Effect>) {
        match event {
            Event::Search { request, result } => self.search_arrived(request, result, effects),
            Event::Graph { request, result } => self.graph_arrived(request, result),
            Event::Answer { request, result } => self.answer_arrived(request, result),
            Event::Page { request, result } => self.page_arrived(request, result),
            Event::PageConcepts { request, result } => self.page_concepts_arrived(request, result),
            Event::Catalogue { request, result } => self.catalogue_arrived(request, result),
            Event::LabelsSaved {
                request,
                doc,
                result,
            } => self.labels_saved(request, doc, result, effects),
            Event::Deleted {
                request,
                doc,
                result,
            } => self.deleted(request, doc, result, effects),
            Event::Preflight { request, result } => self.preflight_arrived(request, result),
            Event::IngestProgress { request, progress } => self.progress_arrived(request, progress),
            Event::IngestFinished { request, result } => {
                self.ingest_finished(request, result, effects)
            }
            Event::Health {
                request,
                service,
                state,
            } => self.health_arrived(request, service, state),
        }
    }

    /// True when nothing is loading, being checked or running.
    pub fn is_at_rest(&self) -> bool {
        !self.ask.is_running()
            && !self.source_is_busy()
            && !self.library_is_busy()
            && self.health.pending.is_none()
            && !self.ingest.is_working()
    }

    pub(super) fn issue_request(&mut self) -> RequestId {
        self.next_request += 1;
        RequestId(self.next_request)
    }

    pub(super) fn add_notice(
        &mut self,
        kind: NoticeKind,
        title: String,
        detail: String,
        failure: Option<crate::contract::Failure>,
    ) {
        self.next_notice += 1;
        self.notices.insert(
            0,
            Notice {
                id: NoticeId(self.next_notice),
                kind,
                title,
                detail,
                failure,
                read: false,
            },
        );
    }

    fn request_quit(&mut self, effects: &mut Vec<Effect>) {
        if self.ingest.is_running() && self.quit == Quit::No {
            self.quit = Quit::Asking;
        } else {
            effects.push(Effect::CloseWindow);
        }
    }

    fn confirm_quit(&mut self, effects: &mut Vec<Effect>) {
        self.quit = Quit::Confirmed;
        if self.ingest.is_running() {
            self.cancel_ingest(effects);
        } else {
            effects.push(Effect::CloseWindow);
        }
    }
}

/// The items of a document in a short sentence part, such as "3 passages and 1 formula".
pub(super) fn counts_text(items: &ItemCounts) -> String {
    fn count(number: u64, noun: &str) -> String {
        match number {
            1 => format!("1 {noun}"),
            _ => format!("{number} {noun}s"),
        }
    }
    format!(
        "{}, {}, {} and {}",
        count(items.chunks, "passage"),
        count(items.formulas, "formula"),
        count(items.figures, "figure"),
        count(items.tables, "table")
    )
}

/// Tells the backend to stop the work of a request.
pub(super) fn push_cancel(effects: &mut Vec<Effect>, request: RequestId) {
    effects.push(Effect::Send(Command::Cancel(request)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{AskDraft, Failure, IngestOutcome, IngestRequest, Loadable, NoticeKind};
    use crate::state::IngestJob;
    use crate::testkit::sample;

    fn run(shared: &mut Shared, intent: Intent) -> Vec<Effect> {
        let mut effects = Vec::new();
        shared.apply_intent(intent, &mut effects);
        effects
    }

    fn running() -> Shared {
        Shared {
            ingest: IngestJob::Running {
                request: IngestRequest::default(),
                id: RequestId(5),
                progress: None,
            },
            ..Shared::default()
        }
    }

    #[test]
    fn quitting_during_an_ingest_asks_first() {
        let mut idle = Shared::default();
        assert_eq!(
            run(&mut idle, Intent::RequestQuit),
            vec![Effect::CloseWindow]
        );
        assert_eq!(idle.quit, Quit::No);

        let mut shared = running();
        assert!(run(&mut shared, Intent::RequestQuit).is_empty());
        assert_eq!(shared.quit, Quit::Asking);
        run(&mut shared, Intent::DismissQuit);
        assert_eq!(shared.quit, Quit::No);
        assert!(shared.ingest.is_running(), "saying no leaves the run alone");

        run(&mut shared, Intent::RequestQuit);
        let effects = run(&mut shared, Intent::ConfirmQuit);
        assert_eq!(shared.quit, Quit::Confirmed);
        assert!(matches!(shared.ingest, IngestJob::Stopping { .. }));
        assert_eq!(effects, vec![Effect::Send(Command::Cancel(RequestId(5)))]);

        let mut forced = Vec::new();
        shared.apply_intent(Intent::RequestQuit, &mut forced);
        assert_eq!(
            forced,
            vec![Effect::CloseWindow],
            "a second close while stopping forces it"
        );

        let mut closing = Vec::new();
        shared.apply_event(
            Event::IngestFinished {
                request: RequestId(5),
                result: Ok(IngestOutcome::Cancelled),
            },
            &mut closing,
        );
        assert_eq!(
            closing,
            vec![Effect::CloseWindow],
            "the window closes when the run has ended"
        );

        let mut finished = Shared::default();
        assert_eq!(
            run(&mut finished, Intent::ConfirmQuit),
            vec![Effect::CloseWindow]
        );
    }

    #[test]
    fn the_plain_intents_change_only_what_they_name() {
        let mut shared = Shared::default();
        run(&mut shared, Intent::OpenTab(Tab::Library));
        assert_eq!(shared.tab, Tab::Library);
        run(&mut shared, Intent::ToggleHelp);
        assert!(shared.help_open);
        run(&mut shared, Intent::FocusAskBar);
        assert_eq!((shared.tab, shared.cues.focus_ask_bar), (Tab::Ask, 1));
        assert_eq!(run(&mut shared, Intent::PickPdf), vec![Effect::PickFile]);
        run(&mut shared, Intent::PdfPicked("/books/a.pdf".into()));
        assert_eq!(shared.cues.pdf_picks, 1);
        assert_eq!(
            shared.cues.picked_pdf.as_deref(),
            Some(std::path::Path::new("/books/a.pdf"))
        );
        assert_eq!(
            run(&mut shared, Intent::CopyText("x".to_owned())),
            vec![Effect::CopyText("x".to_owned())]
        );
        shared.add_notice(NoticeKind::Done, "one".to_owned(), String::new(), None);
        shared.add_notice(NoticeKind::Done, "two".to_owned(), String::new(), None);
        assert_eq!(shared.notices[0].title, "two", "newest first");
        run(&mut shared, Intent::MarkNoticesRead);
        assert!(shared.notices.iter().all(|notice| notice.read));
        let id = shared.notices[0].id;
        run(&mut shared, Intent::DismissNotice(id));
        assert_eq!(shared.notices.len(), 1);
        assert!(shared.is_at_rest());
        run(&mut shared, Intent::RefreshCatalogue);
        assert!(!shared.is_at_rest());
    }

    #[test]
    fn an_ask_that_fails_after_its_results_ends_the_graph_and_the_answer() {
        let mut shared = Shared::default();
        let draft = AskDraft {
            question: "q".to_owned(),
            ..AskDraft::default()
        };
        let effects = run(&mut shared, Intent::Ask(draft));
        let [Effect::Send(ask)] = effects.as_slice() else {
            panic!("expected one command, got {effects:?}");
        };
        let request = ask.request();
        let failure = Failure::internal("the backend stopped");
        let failed = ask.failed(&failure);
        assert_eq!(
            failed,
            vec![
                Event::Search {
                    request,
                    result: Err(failure.clone()),
                },
                Event::Graph {
                    request,
                    result: Err(failure.clone()),
                },
                Event::Answer {
                    request,
                    result: Err(failure.clone()),
                },
            ]
        );

        let found = sample::search_reply();
        let arrived = Event::Search {
            request,
            result: Ok(found.clone()),
        };
        shared.apply_event(arrived, &mut Vec::new());
        for event in failed {
            shared.apply_event(event, &mut Vec::new());
        }
        assert_eq!(
            shared.ask.search,
            Loadable::Ready(found),
            "the results stay"
        );
        assert_eq!(shared.ask.graph, Loadable::Failed(failure.clone()));
        assert_eq!(shared.ask.answer, Loadable::Failed(failure));
        assert!(shared.is_at_rest());
    }
}
