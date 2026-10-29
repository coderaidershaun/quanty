//! The state of the whole window, and the one place that matches each intent and each event to
//! the rule that handles it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{
    AskSession, Balances, Contents, DatasetJob, ExportJob, Health, IngestJob, Insights, Library,
    Shelf, SourceNav,
};
use crate::theme::Scheme;
use contract::{
    AskView, Command, DocId, Effect, Event, Failure, Intent, Loadable, Notice, NoticeId,
    NoticeKind, Panel, RequestId, StartupFacts, Tab,
};

/// Counters that only go up. A panel acts when one is above what it saw last.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Cues {
    pub focus_ask_bar: u64,
    /// The Ingest panel copies `picked_pdf` into its draft.
    pub pdf_picks: u64,
    pub picked_pdf: Option<contract::UploadedPdf>,
    pub pdf_upload_error: Option<String>,
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
    /// The view of the Ask tab that a compact window shows.
    pub ask_view: AskView,
    /// The last view other than the Answer that was chosen. `None` until one is.
    side_view: Option<AskView>,
    /// The Source view was opened from the Library, so Back returns there.
    pub(super) source_from_library: bool,
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
    /// The palette the person chose.
    pub scheme: Scheme,
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
            Intent::ShowAskView(view) => {
                self.open_tab(Tab::Ask);
                self.show_ask_view(view);
            }
            Intent::Back => self.back(effects),
            Intent::FocusAskBar => {
                self.open_tab(Tab::Ask);
                self.ask_view = AskView::Answer;
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
            Intent::SelectResult(number) => self.choose_result(number, effects),
            Intent::StepResult(delta) => self.step_result(delta, effects),
            Intent::FocusConcept(concept) => self.ask.focused_concept = concept,
            Intent::OpenSource { doc, page, piece } => {
                self.show_source(doc, page, piece, effects);
            }
            Intent::TurnPage(delta) => self.turn_page(delta, effects),
            Intent::OpenReader(doc) => self.open_reader(doc, effects),
            Intent::CloseReader => self.shelf.is_reading = false,
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
            Intent::PickPdf => {
                self.cues.pdf_upload_error = None;
                effects.push(Effect::PickFile);
            }
            Intent::PdfPicked(path) => {
                self.cues.pdf_upload_error = None;
                self.cues.picked_pdf = Some(path);
                self.cues.pdf_picks += 1;
            }
            Intent::PdfUploadFailed(error) => self.cues.pdf_upload_error = Some(error),
            Intent::CheckIngest(request) => self.check_ingest(request, effects),
            Intent::StartIngest => self.start_ingest(effects),
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
            Intent::SwitchScheme => self.scheme = self.scheme.other(),
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
        self.source_from_library = false;
    }

    /// The view that a medium window shows beside the Answer. Never the Answer.
    pub fn side_view(&self) -> AskView {
        self.side_view.unwrap_or(AskView::Source)
    }

    /// Whether `Intent::Back` would change anything.
    pub fn can_go_back(&self) -> bool {
        self.back_step().is_some()
    }

    /// Whether `Intent::Back` would only return the Ask tab to its Answer. A window that shows
    /// the Answer beside the other views sees no change from it.
    pub(crate) fn goes_back_to_the_answer(&self) -> bool {
        self.back_step() == Some(BackStep::ShowAnswer)
    }

    fn back_step(&self) -> Option<BackStep> {
        match self.tab {
            Tab::Ask if self.maximised.is_some() => Some(BackStep::Restore),
            Tab::Ask if self.returns_to_library() => Some(BackStep::ReturnToLibrary),
            Tab::Ask if self.ask_view != AskView::Answer => Some(BackStep::ShowAnswer),
            Tab::Ask => None,
            Tab::Library => Some(BackStep::UpTheShelf),
            Tab::Ingest | Tab::Datasets => Some(BackStep::OpenAsk),
        }
    }

    /// Whether the Source view that shows is the one the Library opened.
    pub(super) fn returns_to_library(&self) -> bool {
        self.tab == Tab::Ask && self.ask_view == AskView::Source && self.source_from_library
    }

    /// Any view other than the Answer also becomes the side view. A view that is chosen is not the
    /// Library's any more.
    pub(super) fn show_ask_view(&mut self, view: AskView) {
        self.ask_view = view;
        self.source_from_library = false;
        if AskView::SIDE.contains(&view) {
            self.side_view = Some(view);
        }
    }

    /// Chooses a result that exists and shows it in the Source view. A number with no result
    /// changes nothing.
    fn choose_result(&mut self, number: usize, effects: &mut Vec<Effect>) {
        if self.ask.result(number).is_some() {
            self.select_result(number, effects);
            self.show_ask_view(AskView::Source);
        }
    }

    /// Undoes one step, the first that applies: a maximised panel of the Ask tab is put back, a
    /// Source view that the Library opened returns to the Library, the Ask tab returns to its
    /// Answer, the Library goes up one level, any other tab returns to the Ask tab.
    fn back(&mut self, effects: &mut Vec<Effect>) {
        match self.back_step() {
            Some(BackStep::Restore) => self.maximised = None,
            Some(BackStep::ReturnToLibrary) => self.open_tab(Tab::Library),
            Some(BackStep::ShowAnswer) => self.ask_view = AskView::Answer,
            Some(BackStep::UpTheShelf) => {
                if !self.back_on_shelf(effects) {
                    self.open_tab(Tab::Ask);
                }
            }
            Some(BackStep::OpenAsk) => self.open_tab(Tab::Ask),
            None => {}
        }
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

/// The step that `Intent::Back` takes, the first rule that applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackStep {
    Restore,
    ReturnToLibrary,
    ShowAnswer,
    UpTheShelf,
    OpenAsk,
}

pub(super) fn push_cancel(effects: &mut Vec<Effect>, request: RequestId) {
    effects.push(Effect::Send(Command::Cancel(request)));
}
