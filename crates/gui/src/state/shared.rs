//! The state of the whole window, and the one place that matches each intent and each event to
//! the rule that handles it.

use std::path::PathBuf;

use super::{AskSession, Health, IngestJob, Library, SourceNav};
use crate::contract::{
    Command, Effect, Event, Intent, Notice, NoticeId, NoticeKind, RequestId, StartupFacts, Tab,
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
    pub focus_ask_bar: u64,
    /// The Ingest panel copies `picked_pdf` into its draft.
    pub pdf_picks: u64,
    pub picked_pdf: Option<PathBuf>,
    /// The source view shows its target again.
    pub source_shows: u64,
    /// The Ingest panel chooses the book `saved_book` once the catalogue that holds it arrives.
    pub book_saves: u64,
    pub saved_book: Option<String>,
}

/// `Default` is for tests; the app makes it with `new`.
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
            Intent::SaveBook(book) => self.save_book(book, effects),
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

    /// An event whose request is not the one its slot waits for changes nothing.
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
            Event::BookSaved { request, result } => self.book_saved(request, result, effects),
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

pub(super) fn push_cancel(effects: &mut Vec<Effect>, request: RequestId) {
    effects.push(Effect::Send(Command::Cancel(request)));
}
