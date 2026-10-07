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

    /// Loads the page and its concepts of the open target again. A page that is shown stays
    /// shown until the new one lands.
    pub(super) fn reload_source(&mut self, effects: &mut Vec<Effect>) {
        let Some(target) = self.source.target else {
            return;
        };
        if self.source.page.ready().is_none() {
            self.source.page = Loadable::Loading;
        }
        self.load_page(target.doc, target.page, effects);
    }

    /// Asks the backend for a page and its concepts, and stops the load that was on its way.
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

    /// Closes the source view when it shows this document, and stops the page load for it.
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use uuid::Uuid;

    use super::*;
    use crate::contract::{Book, Catalogue, Document, Event, FailureKind, Intent};

    fn doc(number: u128) -> DocId {
        DocId(Uuid::from_u128(number))
    }

    fn view(doc: DocId, page: u32) -> PageView {
        PageView {
            doc,
            page,
            page_count: 3,
            ..PageView::default()
        }
    }

    fn catalogue_with_folder(doc: DocId) -> Catalogue {
        Catalogue {
            books: vec![Book {
                title: None,
                chapters: vec![Document {
                    id: doc,
                    folder: Some(PathBuf::from("/chapters/one")),
                    ..Document::default()
                }],
            }],
        }
    }

    fn run(shared: &mut Shared, intent: Intent) -> Vec<Effect> {
        let mut effects = Vec::new();
        shared.apply_intent(intent, &mut effects);
        effects
    }

    fn deliver(shared: &mut Shared, event: Event) {
        shared.apply_event(event, &mut Vec::new());
    }

    fn loaded(effects: &[Effect]) -> RequestId {
        match effects {
            [.., Effect::Send(Command::LoadPage { request, .. })] => *request,
            other => panic!("expected a page load, got {other:?}"),
        }
    }

    fn open(doc: DocId, page: u32) -> Intent {
        Intent::OpenSource {
            doc,
            page,
            piece: None,
        }
    }

    fn land(shared: &mut Shared, request: RequestId, view: PageView) {
        deliver(
            shared,
            Event::Page {
                request,
                result: Ok(view),
            },
        );
        deliver(
            shared,
            Event::PageConcepts {
                request,
                result: Ok(Vec::new()),
            },
        );
    }

    #[test]
    fn a_page_stays_shown_until_the_next_one_lands() {
        let (one, two) = (doc(1), doc(2));
        let mut shared = Shared::default();
        shared.library.catalogue = Loadable::Ready(catalogue_with_folder(one));

        assert!(
            run(&mut shared, Intent::TurnPage(1)).is_empty(),
            "no page is shown yet"
        );
        assert!(
            run(&mut shared, Intent::ReloadSource).is_empty(),
            "nothing is open"
        );
        assert_eq!(shared.cues.source_shows, 0);

        let effects = run(
            &mut shared,
            Intent::OpenSource {
                doc: one,
                page: 1,
                piece: Some(2),
            },
        );
        let first = loaded(&effects);
        assert!(matches!(
            effects.as_slice(),
            [Effect::Send(Command::LoadPage { doc: loaded_doc, page: 1, folder: Some(_), .. })]
                if *loaded_doc == one
        ));
        assert_eq!(shared.source.generation, 1);
        assert_eq!(shared.cues.source_shows, 1);
        assert_eq!(shared.source.pending, Some(first));
        assert!(shared.source.page.is_loading() && shared.source.concepts.is_loading());
        land(&mut shared, first, view(one, 1));
        assert!(shared.source.pending.is_none());
        assert_eq!(shared.source.page.ready().map(|shown| shown.page), Some(1));

        let effects = run(
            &mut shared,
            Intent::OpenSource {
                doc: one,
                page: 1,
                piece: Some(5),
            },
        );
        assert!(
            effects.is_empty(),
            "the page is already shown: only the piece changes"
        );
        assert_eq!(
            shared.source.target.and_then(|target| target.piece),
            Some(5)
        );
        assert_eq!(shared.cues.source_shows, 2);
        let again = Intent::OpenSource {
            doc: one,
            page: 1,
            piece: Some(5),
        };
        assert!(run(&mut shared, again).is_empty(), "no second load");
        assert_eq!(
            shared.cues.source_shows, 3,
            "the view shows its target again"
        );

        let effects = run(&mut shared, Intent::TurnPage(1));
        let second = loaded(&effects);
        assert_eq!(effects.len(), 1, "nothing to cancel");
        assert_eq!(
            shared.source.generation, 1,
            "the same document keeps its zoom and scroll"
        );
        assert_eq!(shared.source.page.ready().map(|shown| shown.page), Some(1));
        assert!(shared.source.concepts.is_loading());
        assert_eq!(shared.cues.source_shows, 4, "a page that changes counts");

        let effects = run(&mut shared, Intent::TurnPage(1));
        let third = loaded(&effects);
        assert_eq!(effects[0], Effect::Send(Command::Cancel(second)));
        assert!(
            matches!(
                effects.last(),
                Some(Effect::Send(Command::LoadPage { page: 3, .. }))
            ),
            "a held key counts from the page asked for"
        );
        assert!(
            run(&mut shared, Intent::TurnPage(1)).is_empty(),
            "the chapter ends at page 3"
        );
        assert_eq!(
            shared.cues.source_shows, 5,
            "a turn that changes nothing does not count"
        );

        land(&mut shared, second, view(one, 2));
        assert_eq!(
            shared.source.page.ready().map(|shown| shown.page),
            Some(1),
            "a slow page does not overwrite a newer request"
        );
        land(&mut shared, third, view(one, 3));
        assert_eq!(shared.source.page.ready().map(|shown| shown.page), Some(3));
        assert!(shared.source.pending.is_none());

        // A reload asks for the open page again and keeps what is shown.
        let before = (
            shared.source.generation,
            shared.tab,
            shared.source.target,
            shared.cues.clone(),
        );
        let effects = run(&mut shared, Intent::ReloadSource);
        let reload = loaded(&effects);
        assert!(matches!(
            effects.as_slice(),
            [Effect::Send(Command::LoadPage { doc: reloaded, page: 3, folder: Some(_), .. })]
                if *reloaded == one
        ));
        assert_eq!(shared.source.pending, Some(reload));
        assert_eq!(shared.source.page.ready().map(|shown| shown.page), Some(3));
        assert!(shared.source.concepts.is_loading());
        let after = (
            shared.source.generation,
            shared.tab,
            shared.source.target,
            shared.cues.clone(),
        );
        assert_eq!(
            after, before,
            "a reload changes no generation, tab, target or cue"
        );
        let effects = run(&mut shared, Intent::ReloadSource);
        assert_eq!(effects[0], Effect::Send(Command::Cancel(reload)));
        land(&mut shared, loaded(&effects), view(one, 3));
        assert!(shared.source.pending.is_none());

        let effects = run(&mut shared, open(one, 2));
        let failing = loaded(&effects);
        deliver(
            &mut shared,
            Event::Page {
                request: failing,
                result: Err(Failure::new(FailureKind::SourceMissing, "gone")),
            },
        );
        assert!(shared.source.page.failure().is_some());
        run(&mut shared, Intent::ReloadSource);
        assert!(shared.source.page.is_loading(), "a failed page loads again");
        let effects = run(&mut shared, open(one, 2));
        assert_ne!(
            loaded(&effects),
            failing,
            "a failed page is asked for again"
        );
        assert!(shared.source.page.is_loading());

        let effects = run(&mut shared, open(two, 1));
        let other = loaded(&effects);
        assert_eq!(
            shared.source.generation, 2,
            "another document starts afresh"
        );
        land(&mut shared, other, view(two, 1));
        assert!(
            run(&mut shared, Intent::TurnPage(-1)).is_empty(),
            "page 1 is the first"
        );
        assert_eq!(shared.tab, crate::contract::Tab::Ask);
    }
}
