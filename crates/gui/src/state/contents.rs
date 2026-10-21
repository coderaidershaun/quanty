//! The contents of the open document, as the Code, Formulas, Figures, Tables and Concepts views
//! read them: the items from the disk and the concepts from the graph, so one can be ready while
//! the other failed.

use super::shared::{Shared, push_cancel};
use crate::contract::{ChapterContents, DocId, Effect, Failure, Loadable, PageConcept, RequestId};

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct Contents {
    /// The document the slot was loaded for.
    pub doc: Option<DocId>,
    /// `Some` while the items or the concepts are on the way.
    pub pending: Option<RequestId>,
    pub items: Loadable<ChapterContents>,
    pub concepts: Loadable<Vec<PageConcept>>,
}

impl Shared {
    /// Called when a document opens and when a view is shown. The views read nothing yet, so it
    /// loads nothing.
    pub(super) fn open_view_contents(&mut self, _effects: &mut Vec<Effect>) {}

    /// Cancels a pending request and empties the slot. Called when another media or document
    /// opens and when the open document is gone from the catalogue.
    pub(super) fn forget_contents(&mut self, effects: &mut Vec<Effect>) {
        if let Some(request) = self.contents.pending {
            push_cancel(effects, request);
        }
        self.contents = Contents::default();
    }

    /// Lands only for the pending request. The load ends with the concepts, not here.
    pub(super) fn chapter_contents_arrived(
        &mut self,
        request: RequestId,
        result: Result<ChapterContents, Failure>,
    ) {
        if self.contents.pending != Some(request) {
            return;
        }
        self.contents.items = self.settled(result);
    }

    /// Lands only for the pending request, and ends the load.
    pub(super) fn chapter_concepts_arrived(
        &mut self,
        request: RequestId,
        result: Result<Vec<PageConcept>, Failure>,
    ) {
        if self.contents.pending != Some(request) {
            return;
        }
        self.contents.concepts = self.settled(result);
        self.contents.pending = None;
    }

    pub(super) fn contents_is_busy(&self) -> bool {
        self.contents.pending.is_some()
            || self.contents.items.is_loading()
            || self.contents.concepts.is_loading()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_lands_only_for_the_pending_request_and_the_concepts_end_the_load() {
        let mut shared = Shared::default();
        shared.contents.pending = Some(RequestId(2));
        shared.contents.items = Loadable::Loading;
        shared.contents.concepts = Loadable::Loading;
        assert!(shared.contents_is_busy());

        shared.chapter_contents_arrived(RequestId(1), Ok(ChapterContents::default()));
        shared.chapter_concepts_arrived(RequestId(1), Ok(Vec::new()));
        assert_eq!(shared.contents.items, Loadable::Loading);
        assert_eq!(shared.contents.concepts, Loadable::Loading);
        assert_eq!(shared.contents.pending, Some(RequestId(2)));

        let contents = ChapterContents {
            page_count: 12,
            ..ChapterContents::default()
        };
        shared.chapter_contents_arrived(RequestId(2), Ok(contents.clone()));
        assert_eq!(shared.contents.items, Loadable::Ready(contents));
        assert_eq!(shared.contents.pending, Some(RequestId(2)));
        assert!(
            shared.contents_is_busy(),
            "the concepts are still on the way"
        );

        let concept = PageConcept {
            name: "Delta".to_owned(),
            ..PageConcept::default()
        };
        shared.chapter_concepts_arrived(RequestId(2), Ok(vec![concept.clone()]));
        assert_eq!(shared.contents.concepts, Loadable::Ready(vec![concept]));
        assert_eq!(shared.contents.pending, None);
        assert!(!shared.contents_is_busy());
    }
}
