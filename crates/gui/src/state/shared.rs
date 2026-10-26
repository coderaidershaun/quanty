//! The state of the whole window, and the one place that matches each intent and each event to
//! the rule that handles it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{
    AskSession, Balances, Contents, DatasetJob, ExportJob, Health, IngestJob, Insights, Library,
    Shelf, SourceNav,
};
use contract::{
    Command, DocId, Effect, Event, Failure, Intent, Loadable, Notice, NoticeId, NoticeKind, Panel,
    RequestId, StartupFacts, Tab,
};

/// Counters that only go up. A panel acts when one is above what it saw last.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Cues {
    pub focus_ask_bar: u64,
    /// The Ingest panel copies `picked_pdf` into its draft.
    pub pdf_picks: u64,
    pub picked_pdf: Option<contract::UploadedPdf>,
    /// The source view shows its target again.
    pub source_shows: u64,
    /// The Ingest panel chooses the media `saved_media` once the catalogue that holds it arrives.
    pub media_saves: u64,
    pub saved_media: Option<String>,
}

/// `Default` is for tests; the app makes it with `new`.
#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct Shared {
    pub tab: Tab,
    /// The panel that has the whole Ask tab. The other panels of the tab are not drawn.
    pub maximised: Option<Panel>,
    pub cues: Cues,
    pub ask: AskSession,
    pub source: SourceNav,
    pub library: Library,
    pub shelf: Shelf,
    pub contents: Contents,
    pub insights: Insights,
    pub ingest: IngestJob,
    pub datasets: DatasetJob,
    /// The export being written, or the one that ended last.
    pub export: Option<ExportJob>,
    /// Newest first, this session only.
    pub notices: Vec<Notice>,
    pub health: Health,
    pub balances: Balances,
    pub model_choice_errors: BTreeMap<choices::WorkKind, Failure>,
    model_choice_requests: BTreeMap<choices::WorkKind, RequestId>,
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
        let before = self.paid_jobs();
        match intent {
            Intent::OpenTab(tab) => self.open_tab(tab),
            Intent::Maximise(panel) => self.maximised = Some(panel),
            Intent::RestorePanels => self.maximised = None,
            Intent::StopOrRestore => self.stop_or_restore(effects),
            Intent::FocusAskBar => {
                self.open_tab(Tab::Ask);
                self.cues.focus_ask_bar += 1;
            }
            Intent::CopyText(text) => effects.push(Effect::CopyText(text)),
            Intent::ShareAnswer => {
                if let Some(markdown) = self.ask.answer_markdown() {
                    effects.push(Effect::CopyText(markdown));
                }
            }
            Intent::Export(export) => self.start_export(export, effects),
            Intent::Ask(draft) => self.start_ask(draft, effects),
            Intent::CancelAsk => self.cancel_ask(effects),
            Intent::SelectResult(number) => self.select_result(number, effects),
            Intent::StepResult(delta) => self.step_result(delta, effects),
            Intent::FocusConcept(concept) => self.ask.focused_concept = concept,
            Intent::OpenSource { doc, page, piece } => self.open_source(doc, page, piece, effects),
            Intent::TurnPage(delta) => self.turn_page(delta, effects),
            Intent::ReloadSource => self.reload_source(effects),
            Intent::OpenMedia { title } => self.open_media(title, effects),
            Intent::OpenDocument(doc) => self.open_document(doc, effects),
            Intent::ShowDocumentView(view) => self.show_document_view(view, effects),
            Intent::RefreshCatalogue => self.refresh_catalogue(effects),
            Intent::SetDocumentTags(edit) => self.set_document_tags(edit, effects),
            Intent::DeleteDocument(doc) => self.delete_document(doc, effects),
            Intent::SaveMedia(media) => self.save_media(media, effects),
            Intent::EditMedia(edit) => self.edit_media(edit, effects),
            Intent::DeleteMedia(title) => self.delete_media(title, effects),
            Intent::PickPdf => effects.push(Effect::PickFile),
            Intent::PdfPicked(path) => {
                self.cues.picked_pdf = Some(path);
                self.cues.pdf_picks += 1;
            }
            Intent::CheckIngest(request) => self.check_ingest(request, effects),
            Intent::StartIngest => self.start_ingest(effects),
            Intent::CancelIngest => self.cancel_ingest(effects),
            Intent::ClearIngest => self.clear_ingest(effects),
            Intent::StartDatasetBuild(request) => self.start_dataset_build(request, effects),
            Intent::ClearDatasetBuild => self.clear_dataset_build(),
            Intent::DismissNotice(id) => self.notices.retain(|notice| notice.id != id),
            Intent::RecheckHealth => self.recheck_health(effects),
            Intent::ChooseModel { work, choice } => {
                *self.health.facts.model_choices.of_mut(work) = choice.clone();
                let request = self.issue_request();
                self.model_choice_requests.insert(work, request);
                self.model_choice_errors.remove(&work);
                effects.push(Effect::Send(Command::SaveModelChoice {
                    request,
                    work,
                    choice,
                }));
            }
            Intent::ReadBalances => self.read_balances(effects),
            Intent::OpenBillingPage(provider) => {
                effects.push(Effect::OpenUrl(provider.billing_page().to_owned()));
            }
        }
        self.read_after_a_paid_job(before, effects);
    }

    /// An event whose request is not the one its slot waits for changes nothing.
    pub fn apply_event(&mut self, event: Event, effects: &mut Vec<Effect>) {
        let before = self.paid_jobs();
        self.hear_paid_reply(&event);
        match event {
            Event::ModelChoiceSaved { request, result } => {
                if let Some(work) = self
                    .model_choice_requests
                    .iter()
                    .find_map(|(work, pending)| (*pending == request).then_some(*work))
                {
                    self.model_choice_requests.remove(&work);
                    if let Err(failure) = result {
                        self.model_choice_errors.insert(work, failure);
                    }
                }
            }
            Event::Search { request, result } => self.search_arrived(request, result, effects),
            Event::Graph { request, result } => self.graph_arrived(request, result),
            Event::Answer { request, result } => self.answer_arrived(request, result),
            Event::Exported {
                request,
                format,
                result,
            } => self.export_finished(request, format, result, effects),
            Event::Page { request, result } => self.page_arrived(request, result),
            Event::PageConcepts { request, result } => self.page_concepts_arrived(request, result),
            Event::ChapterContents { request, result } => {
                self.chapter_contents_arrived(request, result);
            }
            Event::ChapterConcepts { request, result } => {
                self.chapter_concepts_arrived(request, result);
            }
            Event::ChapterInsights { request, result } => {
                self.chapter_insights_arrived(request, result);
            }
            Event::Catalogue { request, result } => {
                self.catalogue_arrived(request, result, effects);
            }
            Event::DocumentTagsSaved {
                request,
                doc,
                result,
            } => self.document_tags_saved(request, doc, result, effects),
            Event::DocumentDeleted {
                request,
                doc,
                result,
            } => self.document_deleted(request, doc, result, effects),
            Event::MediaSaved { request, result } => self.media_saved(request, result, effects),
            Event::MediaEdited { request, result } => self.media_edited(request, result, effects),
            Event::MediaDeleted { request, result } => {
                self.media_deleted(request, result, effects);
            }
            Event::Preflight { request, result } => self.preflight_arrived(request, result),
            Event::IngestProgress { request, progress } => self.progress_arrived(request, progress),
            Event::IngestFinished { request, result } => {
                self.ingest_finished(request, result, effects)
            }
            Event::DatasetProgress { request, progress } => {
                self.dataset_progress_arrived(request, progress)
            }
            Event::DatasetFinished { request, result } => {
                self.dataset_build_finished(request, result)
            }
            Event::Health {
                request,
                service,
                state,
            } => self.health_arrived(request, service, state),
            Event::Balances { request, found } => self.balances_arrived(request, found),
        }
        self.read_after_a_paid_job(before, effects);
    }

    pub fn is_at_rest(&self) -> bool {
        !self.ask.is_running()
            && !self.is_exporting()
            && !self.source_is_busy()
            && !self.library_is_busy()
            && !self.contents_is_busy()
            && !self.insights_is_busy()
            && self.health.pending.is_none()
            && !self.ingest.is_working()
            && !self.datasets.is_running()
            && !self.balances.is_reading()
    }

    pub(super) fn issue_request(&mut self) -> RequestId {
        self.next_request += 1;
        RequestId(self.next_request)
    }

    /// Cancels the request that is still pending, if any, and issues the one that replaces it.
    pub(super) fn restart(
        &mut self,
        pending: Option<RequestId>,
        effects: &mut Vec<Effect>,
    ) -> RequestId {
        if let Some(old) = pending {
            push_cancel(effects, old);
        }
        self.issue_request()
    }

    /// The folder on disk of a document of the catalogue, when the catalogue has it.
    pub(super) fn folder_of(&self, doc: DocId) -> Option<PathBuf> {
        self.library
            .catalogue
            .ready()
            .and_then(|catalogue| catalogue.document(doc))
            .and_then(|document| document.folder.clone())
    }

    pub(super) fn add_notice(
        &mut self,
        kind: NoticeKind,
        title: String,
        detail: String,
    ) -> NoticeId {
        self.next_notice += 1;
        let id = NoticeId(self.next_notice);
        self.notices.insert(
            0,
            Notice {
                id,
                kind,
                title,
                detail,
            },
        );
        id
    }

    /// A failure also marks its service down, so a failed panel and the health display agree.
    pub(super) fn settled<T>(&mut self, result: Result<T, Failure>) -> Loadable<T> {
        if let Err(failure) = &result {
            self.mark_down(failure);
        }
        result.into()
    }

    /// A tab that a person opens has every panel in its place. A result or a document that is
    /// opened from inside a maximised panel does not come here, so the panel stays maximised.
    fn open_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.maximised = None;
    }

    /// A running ask is stopped first, so Escape still stops it while a panel is maximised.
    fn stop_or_restore(&mut self, effects: &mut Vec<Effect>) {
        if self.ask.is_running() {
            self.cancel_ask(effects);
        } else {
            self.maximised = None;
        }
    }
}

pub(super) fn push_cancel(effects: &mut Vec<Effect>, request: RequestId) {
    effects.push(Effect::Send(Command::Cancel(request)));
}
