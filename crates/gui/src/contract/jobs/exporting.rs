//! What a panel asks an export to write, where in the window it was asked from, and what the
//! backend is told to write.

use std::path::PathBuf;

use export::{Document, Format};

/// What a panel asks to export: the document as a kind of file, the name the save dialog offers
/// with its extension, and the place in the window that asked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExportRequest {
    pub format: Format,
    pub document: Document,
    pub file_name: String,
    pub place: ExportPlace,
}

/// What the backend writes: the document as that kind of file at that path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExportFile {
    pub format: Format,
    pub document: Document,
    pub path: PathBuf,
}

/// The place in the window that asked for an export, so that the notice of how it ended shows
/// there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExportPlace {
    /// The written answer of the ask with this generation.
    Answer { generation: u64 },
}

impl ExportPlace {
    /// What the notice calls the thing that was exported.
    pub fn what(self) -> &'static str {
        match self {
            ExportPlace::Answer { .. } => "the answer",
        }
    }
}
