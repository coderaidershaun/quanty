//! The saved insights of the open document, as the Insights view reads them.

use export::{Format, InsightsSheet, SheetInsight, insights_document};

use super::shared::{Shared, push_cancel};
use contract::{
    Catalogue, ChapterInsights, Command, DocId, DocumentView, Effect, ExportPlace, ExportRequest,
    Failure, Loadable, RequestId,
};

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct Insights {
    /// The document the slot was loaded for.
    pub doc: Option<DocId>,
    /// `Some` while the insights are on the way.
    pub pending: Option<RequestId>,
    /// `Ready(None)`: nothing is saved for the document yet.
    pub saved: Loadable<Option<ChapterInsights>>,
}

impl Insights {
    /// What the Insights view asks of an export as `format`: the saved insights as a document, a
    /// file name made from the document's title, and the place that asked. `None` while no
    /// insights are saved in the slot, or the catalogue does not list its document.
    pub fn export_request(&self, catalogue: &Catalogue, format: Format) -> Option<ExportRequest> {
        let doc = self.doc?;
        let saved = self.saved.ready()?.as_ref()?;
        let title = &catalogue.document(doc)?.title;
        let sheet = InsightsSheet {
            document_title: title.clone(),
            model: saved.model.clone(),
            insights: saved
                .insights
                .iter()
                .map(|insight| SheetInsight {
                    text: insight.text.clone(),
                    pages: insight.pages.clone(),
                })
                .collect(),
            takeaways: saved.takeaways.clone(),
            cheat_sheet: saved.cheat_sheet.clone(),
        };
        Some(ExportRequest {
            format,
            document: insights_document(&sheet),
            file_name: format.file_name(&format!("{title} insights")),
            place: ExportPlace::Insights { doc },
        })
    }
}

impl Shared {
    /// Called when a document opens and when a view is shown. The Insights view reads the saved
    /// insights of the open document unless the slot holds them or a read of them is on its way,
    /// so showing the view again reads a file that was missing or could not be read, as after the
    /// command that writes it has run. What the slot shows stays until the new answer comes. A
    /// part of a book that gets none is not read.
    pub(super) fn open_view_insights(&mut self, effects: &mut Vec<Effect>) {
        if self.shelf.view != DocumentView::Insights {
            return;
        }
        let Some(doc) = self.shelf.document() else {
            return;
        };
        let catalogue = self.library.catalogue.ready();
        if catalogue.is_some_and(|catalogue| !catalogue.gets_insights(doc)) {
            return;
        }
        let is_same_document = self.insights.doc == Some(doc);
        let is_saved = matches!(self.insights.saved, Loadable::Ready(Some(_)));
        if is_same_document && (is_saved || self.insights.pending.is_some()) {
            return;
        }
        let folder = catalogue
            .and_then(|catalogue| catalogue.document(doc))
            .and_then(|document| document.folder.clone());
        if let Some(old) = self.insights.pending {
            push_cancel(effects, old);
        }
        let request = self.issue_request();
        if !(is_same_document && self.insights.saved.ready().is_some()) {
            self.insights.saved = Loadable::Loading;
        }
        self.insights.doc = Some(doc);
        self.insights.pending = Some(request);
        effects.push(Effect::Send(Command::LoadChapterInsights {
            request,
            doc,
            folder,
        }));
    }

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
    use std::path::PathBuf;

    use uuid::Uuid;

    use super::*;
    use contract::{
        Catalogue, ChapterLabel, Command, Document, DocumentView, Event, Intent, Media,
    };

    fn doc(number: u128) -> DocId {
        DocId(Uuid::from_u128(number))
    }

    fn folder(number: u128) -> PathBuf {
        PathBuf::from(format!("/library/book/chapter-{number}"))
    }

    /// A book with two chapters that have folders, and a part of the book that is not a chapter.
    fn book() -> Catalogue {
        let chapter = |number: u128| Document {
            id: doc(number),
            chapter: Some(ChapterLabel {
                number: u32::try_from(number).expect("a small number"),
                name: "Greeks".to_owned(),
            }),
            folder: Some(folder(number)),
            ..Document::default()
        };
        let index = Document {
            id: doc(3),
            title: "Index".to_owned(),
            ..Document::default()
        };
        Catalogue {
            media: vec![Media {
                title: Some("Book".to_owned()),
                documents: vec![chapter(1), chapter(2), index],
                ..Media::default()
            }],
        }
    }

    fn loads(request: u64, number: u128) -> Effect {
        Effect::Send(Command::LoadChapterInsights {
            request: RequestId(request),
            doc: doc(number),
            folder: Some(folder(number)),
        })
    }

    fn reply(shared: &mut Shared, request: u64, result: Result<Option<ChapterInsights>, Failure>) {
        let request = RequestId(request);
        shared.apply_event(Event::ChapterInsights { request, result }, &mut Vec::new());
    }

    fn show_insights(shared: &mut Shared) -> Vec<Effect> {
        let mut effects = Vec::new();
        shared.apply_intent(
            Intent::ShowDocumentView(DocumentView::Insights),
            &mut effects,
        );
        effects
    }

    #[test]
    fn the_insights_view_reads_the_open_document_until_its_insights_are_saved() {
        let mut shared = Shared::default();
        shared.library.catalogue = Loadable::Ready(book());
        let mut effects = Vec::new();

        shared.apply_intent(Intent::OpenDocument(doc(1)), &mut effects);
        assert!(effects.is_empty(), "the Overview reads no insights");

        assert_eq!(show_insights(&mut shared), [loads(1, 1)]);
        assert_eq!(shared.insights.saved, Loadable::Loading);
        assert_eq!(
            show_insights(&mut shared),
            [],
            "a read on its way is not asked again"
        );

        let saved = ChapterInsights {
            model: "sonnet".to_owned(),
            ..ChapterInsights::default()
        };
        reply(&mut shared, 1, Ok(Some(saved.clone())));
        assert_eq!(shared.insights.saved, Loadable::Ready(Some(saved)));
        assert_eq!(
            show_insights(&mut shared),
            [],
            "saved insights are not read again"
        );

        effects.clear();
        shared.apply_intent(Intent::OpenDocument(doc(2)), &mut effects);
        assert_eq!(
            effects,
            [loads(2, 2)],
            "another document reads its own file, with nothing to cancel"
        );

        reply(&mut shared, 2, Ok(None));
        assert_eq!(shared.insights.saved, Loadable::Ready(None));
        assert_eq!(
            show_insights(&mut shared),
            [loads(3, 2)],
            "a file that was missing is read again"
        );
        assert_eq!(
            shared.insights.saved,
            Loadable::Ready(None),
            "what was shown stays until the answer comes"
        );

        let failure = Failure::not_built("the insights");
        reply(&mut shared, 3, Err(failure.clone()));
        assert_eq!(shared.insights.saved, Loadable::Failed(failure));
        assert_eq!(
            show_insights(&mut shared),
            [loads(4, 2)],
            "a file that could not be read is read again"
        );
        assert_eq!(shared.insights.saved, Loadable::Loading);

        reply(&mut shared, 4, Ok(None));
        effects.clear();
        shared.apply_intent(Intent::OpenDocument(doc(3)), &mut effects);
        assert!(effects.is_empty(), "a part of a book is never read");
        assert_eq!(shared.insights, Insights::default());
    }

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
