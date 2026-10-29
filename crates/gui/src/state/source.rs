//! The page the person is reading beside the answer, and the rules that keep it on screen while
//! the next one loads.

use super::shared::{Shared, push_cancel};
use contract::{
    AskView, Command, DocId, Effect, Failure, Loadable, PageConcept, PageView, RequestId, Tab,
};

/// What was asked for, which may be a page that has not arrived yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SourceTarget {
    pub doc: DocId,
    pub page: u32,
    pub piece: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SourceNav {
    /// Goes up by one when the document changes, so zoom and scroll reset.
    pub generation: u64,
    pub target: Option<SourceTarget>,
    /// `Some` while a page or its concepts are on the way.
    pub pending: Option<RequestId>,
    /// The old page stays `Ready` until the new one lands.
    pub page: Loadable<PageView>,
    pub concepts: Loadable<Vec<PageConcept>>,
}

impl Shared {
    /// What a request for a page does: it opens in the reader while the Library shows one, and
    /// else in the Source view of the Ask tab, which Back returns from to the Library if the
    /// request came from there.
    pub(super) fn show_source(
        &mut self,
        doc: DocId,
        page: u32,
        piece: Option<u32>,
        effects: &mut Vec<Effect>,
    ) {
        if self.is_reader_shown() {
            self.open_source(doc, page, piece, effects);
            return;
        }
        // Choosing another page inside the Source view keeps where it was opened from.
        let from_library = self.tab == Tab::Library || self.returns_to_library();
        self.tab = Tab::Ask;
        self.open_source(doc, page, piece, effects);
        self.show_ask_view(AskView::Source);
        self.source_from_library = from_library;
    }

    /// Opens the reader of the Library on the first page of `doc`.
    pub(super) fn open_reader(&mut self, doc: DocId, effects: &mut Vec<Effect>) {
        self.shelf.is_reading = true;
        self.open_source(doc, 1, None, effects);
    }

    /// True while the Library shows its reader, where a page that is asked for opens.
    fn is_reader_shown(&self) -> bool {
        self.tab == Tab::Library && self.shelf.is_reading
    }

    /// Asks for the page and leaves the tab as it is, so the caller chooses where it shows.
    pub(super) fn open_source(
        &mut self,
        doc: DocId,
        page: u32,
        piece: Option<u32>,
        effects: &mut Vec<Effect>,
    ) {
        self.cues.source_shows += 1;
        let another_document = self.source.target.map(|target| target.doc) != Some(doc);
        let is_shown = self.source.pending.is_none()
            && self
                .source
                .page
                .ready()
                .is_some_and(|view| view.doc == doc && view.page == page);
        self.source.target = Some(SourceTarget { doc, page, piece });
        if is_shown {
            return;
        }
        if another_document {
            self.source.generation += 1;
        }
        if another_document || self.source.page.ready().is_none() {
            self.source.page = Loadable::Loading;
        }
        self.load_page(doc, page, effects);
    }

    pub(super) fn reload_source(&mut self, effects: &mut Vec<Effect>) {
        let Some(target) = self.source.target else {
            return;
        };
        if self.source.page.ready().is_none() {
            self.source.page = Loadable::Loading;
        }
        self.load_page(target.doc, target.page, effects);
    }

    fn load_page(&mut self, doc: DocId, page: u32, effects: &mut Vec<Effect>) {
        let request = self.restart(self.source.pending, effects);
        self.source.pending = Some(request);
        self.source.concepts = Loadable::Loading;
        let folder = self.folder_of(doc);
        effects.push(Effect::Send(Command::LoadPage {
            request,
            doc,
            page,
            folder,
        }));
    }

    /// The turned page shows in the reader of the Library while it is open, and else on the Ask
    /// tab, so a shortcut never turns a page that is out of sight.
    pub(super) fn turn_page(&mut self, delta: i32, effects: &mut Vec<Effect>) {
        let (Some(target), Some(shown)) = (self.source.target, self.source.page.ready()) else {
            return;
        };
        let last = i64::from(shown.page_count.max(1));
        let wanted = (i64::from(target.page) + i64::from(delta)).clamp(1, last);
        let Ok(page) = u32::try_from(wanted) else {
            return;
        };
        if page != target.page {
            if !self.is_reader_shown() {
                self.tab = Tab::Ask;
            }
            self.open_source(target.doc, page, None, effects);
        }
    }

    pub(super) fn page_arrived(&mut self, request: RequestId, result: Result<PageView, Failure>) {
        if self.source.pending != Some(request) {
            return;
        }
        self.source.page = self.settled(result);
    }

    pub(super) fn page_concepts_arrived(
        &mut self,
        request: RequestId,
        result: Result<Vec<PageConcept>, Failure>,
    ) {
        if self.source.pending != Some(request) {
            return;
        }
        self.source.concepts = self.settled(result);
        self.source.pending = None;
    }

    pub(super) fn close_source_of(&mut self, doc: DocId, effects: &mut Vec<Effect>) {
        if self.source.target.map(|target| target.doc) != Some(doc) {
            return;
        }
        if let Some(request) = self.source.pending {
            push_cancel(effects, request);
        }
        self.source = SourceNav {
            generation: self.source.generation + 1,
            ..SourceNav::default()
        };
    }

    pub(super) fn source_is_busy(&self) -> bool {
        self.source.pending.is_some()
            || self.source.page.is_loading()
            || self.source.concepts.is_loading()
    }
}
