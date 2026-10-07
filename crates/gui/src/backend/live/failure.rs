//! Turns each error of the backend into a `Failure`. Every conversion keeps the whole chain of
//! causes as the detail; none of them picks a kind yet.

use std::error::Error;

use graph::GraphError;
use ocr::{ContentError, ConvertError, ReadChapterError};
use rag_core::{ClaudeCliError, ConfigError, EmbedError, LlmError, StoreError};
use rag_ingestion::{ConceptError, DeleteError, IngestError, PdfError, RelabelError};
use rag_retrieval::{AnswerError, SearchError};

use crate::contract::Failure;

/// The error and each cause under it, on one line.
fn chain(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}

macro_rules! failures_from {
    ($($error:ty),+ $(,)?) => {
        $(impl From<$error> for Failure {
            fn from(error: $error) -> Failure {
                Failure::internal(chain(&error))
            }
        })+
    };
}

failures_from!(
    StoreError,
    GraphError,
    EmbedError,
    LlmError,
    ClaudeCliError,
    SearchError,
    AnswerError,
    IngestError,
    PdfError,
    DeleteError,
    RelabelError,
    ConceptError,
    ConvertError,
    ContentError,
    ReadChapterError,
    ConfigError,
);
