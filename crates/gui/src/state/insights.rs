//! The saved insights of the open document, as the Insights, Takeaways and Cheat sheet views read
//! them.

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
    /// What the Insights, Takeaways or Cheat sheet view asks of an export as `format`: the saved
    /// insights as one document whichever of the three is shown, a file name made from the
    /// document's title, and the place that asked. `None` while no insights are saved in the slot,
    /// or the catalogue does not list its document.
    pub fn export_request(
        &self,
        catalogue: &Catalogue,
        view: DocumentView,
        format: Format,
    ) -> Option<ExportRequest> {
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
            // SMELL: any view is taken, so a caller could file the insights under another tab. Only
            // the Insights, Takeaways and Cheat sheet views should be passed.
            place: ExportPlace::Library { doc, view },
        })
    }
}

impl Shared {
    /// Called when a document opens and when a view is shown. The Insights, Takeaways and Cheat
    /// sheet views read the saved insights of the open document unless the slot holds them or a
    /// read of them is on its way, so showing a view again reads a file that was missing or could
    /// not be read, as after the command that writes it has run. What the slot shows stays until
    /// the new answer comes. A part of a book that gets none is not read.
    pub(super) fn open_view_insights(&mut self, effects: &mut Vec<Effect>) {
        if !self.shelf.view.reads_insights() {
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
        let folder = self.folder_of(doc);
        let request = self.restart(self.insights.pending, effects);
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
