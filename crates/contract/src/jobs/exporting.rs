//! What a panel asks an export to write, where in the window it was asked from, and what the
//! backend is told to write.

use export::{Document, Format};

use crate::{DocId, DocumentView};

/// An export request carries its download name and the place that will show its result.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct ExportRequest {
    pub format: Format,
    pub document: Document,
    pub file_name: String,
    pub place: ExportPlace,
}

/// The backend writes this document using the requested format and file name.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct ExportFile {
    pub format: Format,
    pub document: Document,
    pub file_name: String,
}

/// The place in the window that asked for an export, so that the notice of how it ended shows
/// there.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum ExportPlace {
    /// The written answer of the ask with this generation.
    Answer { generation: u64 },
    /// This view of this document in the Library.
    Library { doc: DocId, view: DocumentView },
}

impl ExportPlace {
    /// What the notice calls the thing that was exported.
    pub fn what(self) -> &'static str {
        match self {
            ExportPlace::Answer { .. } => "the answer",
            ExportPlace::Library { view, .. } => match view {
                DocumentView::Insights => "the insights",
                DocumentView::Takeaways => "the takeaways",
                DocumentView::CheatSheet => "the cheat sheet",
                DocumentView::Code => "the code",
                DocumentView::Formulas => "the formulas",
                DocumentView::Figures => "the figures",
                DocumentView::Tables => "the tables",
                DocumentView::Concepts => "the concepts",
            },
        }
    }
}

/// A file that the browser can download from the server.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Download {
    pub url: String,
    pub file_name: String,
}
