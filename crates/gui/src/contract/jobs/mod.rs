//! The long jobs a person starts and watches: the ingest of a PDF and the build of a training
//! dataset.

mod datasets;
mod ingest;

pub use datasets::{DatasetProgress, DatasetReport, DatasetRequest, DatasetStage};
pub use ingest::{
    ChapterState, IngestOutcome, IngestProgress, IngestReport, IngestRequest, IngestStage,
    PageToCheck, Preflight,
};
