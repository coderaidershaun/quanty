//! The long jobs a person starts and watches: the ingest of a PDF, the build of a training
//! dataset, and the export of a document to a file.

mod datasets;
mod exporting;
mod ingest;

pub use datasets::{DatasetProgress, DatasetReport, DatasetRequest, DatasetStage};
pub use exporting::{Download, ExportFile, ExportPlace, ExportRequest};
pub use ingest::{
    ChapterState, IngestOutcome, IngestProgress, IngestReport, IngestRequest, IngestStage,
    InsightsOutcome, PageToCheck, Preflight, UploadedPdf,
};
