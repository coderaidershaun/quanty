//! The page the person is reading beside the answer, and the rules that keep it on screen while
//! the next one loads.

use super::shared::{Shared, push_cancel};
use crate::contract::{
    Command, DocId, Effect, Failure, Loadable, PageConcept, PageView, RequestId, Tab,
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
    pub(super) fn open_source(
        &mut self,
        doc: DocId,
        page: u32,
        piece: Option<u32>,
        effects: &mut Vec<Effect>,
    ) {
        self.tab = Tab::Ask;
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
        let request = self.issue_request();
        if let Some(old) = self.source.pending {
            push_cancel(effects, old);
        }
        self.source.pending = Some(request);
        self.source.concepts = Loadable::Loading;
        let folder = self
            .library
            .catalogue
            .ready()
            .and_then(|catalogue| catalogue.document(doc))
            .and_then(|document| document.folder.clone());
        effects.push(Effect::Send(Command::LoadPage {
            request,
            doc,
            page,
            folder,
        }));
    }

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
            self.open_source(target.doc, page, None, effects);
        }
    }

    pub(super) fn page_arrived(&mut self, request: RequestId, result: Result<PageView, Failure>) {
        if self.source.pending != Some(request) {
            return;
        }
        if let Err(failure) = &result {
            self.mark_down(failure);
        }
        self.source.page = result.into();
    }

    pub(super) fn page_concepts_arrived(
        &mut self,
        request: RequestId,
        result: Result<Vec<PageConcept>, Failure>,
    ) {
        if self.source.pending != Some(request) {
            return;
        }
        if let Err(failure) = &result {
            self.mark_down(failure);
        }
        self.source.concepts = result.into();
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
