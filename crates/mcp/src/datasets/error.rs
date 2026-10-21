//! Everything that stops a dataset build: an argument that is refused, a job that cannot start, and
//! a build that fails while it runs.

use graph::GraphError;
use rag_core::LlmError;
use rag_dataset::BuildError;
use rag_dataset::documents::SelectionError;
use tokio::task::JoinError;

use crate::jobs::{Busy, KEPT_JOBS};

#[derive(thiserror::Error, Debug)]
pub(crate) enum DatasetToolError {
    #[error(
        "`name` is blank; give the name of the dataset, made of letters, digits, '-', '_' and '.', such as \"options-v1\""
    )]
    BlankName,

    #[error(
        "`name` must name a folder, and {name:?} cannot; use letters, digits, '-', '_' and '.', and not '.' or '..'"
    )]
    Name { name: String },

    #[error(
        "`media` names {entry:?}, which is no media in the content folder; give the title of a media, whatever its capitals, as `list_documents` gives it"
    )]
    UnknownMedia { entry: String },

    #[error(
        "`documents` names {entry:?}, which is no document in the content folder; give a `document_id` or the whole `title` from `list_documents`"
    )]
    UnknownDocument { entry: String },

    #[error(
        "`holdout` names {entry:?}, which is no document in the content folder; give a `document_id` or the whole `title` from `list_documents`"
    )]
    UnknownHoldout { entry: String },

    #[error(
        "there is no document to build from; check that the media and documents named are converted to the end, and give `include_book_parts: true` for a part of a book that is not a chapter"
    )]
    NothingToExport,

    #[error("could not connect to the graph")]
    Graph(#[from] GraphError),

    #[error(
        "the model of the Datasets choice is not ready to write the dataset; sign it in, or give `dry_run: true`, which asks no model"
    )]
    ModelNotReady(#[source] LlmError),

    #[error(
        "a dataset build is running (job {job_id}); ask `dataset_status` with that job_id, and start this build when that job has ended"
    )]
    Busy { job_id: String },

    #[error(
        "no dataset build has the job id {job_id:?}; job ids are forgotten when the server restarts, and the server keeps only its last {KEPT_JOBS} builds, so call `build_dataset` again: a dry run is free, and the answers a model already gave are kept and not paid for twice"
    )]
    UnknownJob { job_id: String },

    #[error(
        "the dataset build stopped before it reported; call `build_dataset` again: the answers a model already gave are kept and not paid for twice"
    )]
    Lost,

    /// The finished text of a job that failed, which already says what to do.
    #[error("{0}")]
    Failed(String),

    #[error("could not build the dataset")]
    Build(#[source] Box<BuildError>),

    #[error(
        "the dataset build stopped without an answer; call `build_dataset` again: the answers a model already gave are kept and not paid for twice"
    )]
    Stopped(#[from] JoinError),
}

impl From<Busy> for DatasetToolError {
    fn from(busy: Busy) -> DatasetToolError {
        DatasetToolError::Busy {
            job_id: busy.running_job.to_string(),
        }
    }
}

/// A refusal of the name or of the selection is worded for an agent, which sends arguments and not
/// flags. A model that is not ready gets the words of the tool's own check. Every other failure
/// keeps the words of the build.
impl From<BuildError> for DatasetToolError {
    fn from(error: BuildError) -> DatasetToolError {
        match error {
            BuildError::Name { name } => DatasetToolError::Name { name },
            BuildError::Selection(SelectionError::UnknownMedia { entry }) => {
                DatasetToolError::UnknownMedia { entry }
            }
            BuildError::Selection(SelectionError::UnknownDocument { entry }) => {
                DatasetToolError::UnknownDocument { entry }
            }
            BuildError::Selection(SelectionError::UnknownHoldout { entry }) => {
                DatasetToolError::UnknownHoldout { entry }
            }
            BuildError::NothingToExport => DatasetToolError::NothingToExport,
            // The model can sign out after the tool's check and before the build's own check.
            BuildError::ModelNotReady { source } => DatasetToolError::ModelNotReady(source),
            other => DatasetToolError::Build(Box::new(other)),
        }
    }
}
