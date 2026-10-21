//! The saved insights of the open document, as the Insights view reads them.

use super::shared::{Shared, push_cancel};
use crate::contract::{ChapterInsights, DocId, Effect, Failure, Loadable, RequestId};

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct Insights {
    /// The document the slot was loaded for.
    pub doc: Option<DocId>,
    /// `Some` while the insights are on the way.
    pub pending: Option<RequestId>,
    /// `Ready(None)`: nothing is saved for the document yet.
    pub saved: Loadable<Option<ChapterInsights>>,
}

impl Shared {
    /// Called when a document opens and when a view is shown. The view reads nothing yet, so it
    /// loads nothing.
    pub(super) fn open_view_insights(&mut self, _effects: &mut Vec<Effect>) {}

    /// Cancels a pending request and empties the slot. Called when another media or document
    /// opens and when the open document is gone from the catalogue.
    pub(super) fn forget_insights(&mut self, effects: &mut Vec<Effect>) {
        if let Some(request) = self.insights.pending {
            push_cancel(effects, request);
        }
        self.insights = Insights::default();
    }

    /// Lands only for the pending request, and ends the load.
    pub(super) fn chapter_insights_arrived(
        &mut self,
        request: RequestId,
        result: Result<Option<ChapterInsights>, Failure>,
    ) {
        if self.insights.pending != Some(request) {
            return;
        }
        self.insights.saved = self.settled(result);
        self.insights.pending = None;
    }

    pub(super) fn insights_is_busy(&self) -> bool {
        self.insights.pending.is_some() || self.insights.saved.is_loading()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_lands_only_for_the_pending_request() {
        let mut shared = Shared::default();
        shared.insights.pending = Some(RequestId(2));
        shared.insights.saved = Loadable::Loading;
        assert!(shared.insights_is_busy());

        shared.chapter_insights_arrived(RequestId(1), Ok(None));
        assert_eq!(shared.insights.saved, Loadable::Loading);
        assert_eq!(shared.insights.pending, Some(RequestId(2)));

        let saved = ChapterInsights {
            takeaways: vec!["Hedge delta first.".to_owned()],
            ..ChapterInsights::default()
        };
        shared.chapter_insights_arrived(RequestId(2), Ok(Some(saved.clone())));
        assert_eq!(shared.insights.saved, Loadable::Ready(Some(saved)));
        assert_eq!(shared.insights.pending, None);
        assert!(!shared.insights_is_busy());
    }
}
