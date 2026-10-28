//! Words a failed tool call for the agent: the one place that turns every error of a tool into the
//! text a client shows, and says when that text points at the `health` tool.

use std::error::Error;

use ocr::convert::services::JevError;
use ocr::{ConvertError, PageError};
use rag_core::{EmbedError, error_chain};
use rag_ingestion::{ConceptError, IngestError, PdfError};
use rmcp::ErrorData;
use rmcp::handler::server::tool::IntoCallToolResult;
use rmcp::model::{CallToolResponse, CallToolResult, ContentBlock};

use super::health::{GEMINI_BILLING_PAGE, JEV_CONSOLE_PAGE, LOCATION_REFUSED_DETAIL};
use crate::datasets::DatasetToolError;
use crate::ingest::PdfIngestError;
use crate::insights::ExportToolError;
use crate::insights::InsightsToolError;
use crate::jobs::{StopError, WatchError};
use crate::library::LibraryError;
use crate::retrieval::RetrievalError;

/// Added to the text of a failure of a store or of an outside service.
const HEALTH_HINT: &str = "Call the `health` tool to see which service is not ready.";

/// Why a tool call failed. It reaches the agent as a tool error, with a text that says what to do.
#[derive(thiserror::Error, Debug)]
pub(super) enum ToolError {
    #[error(transparent)]
    Retrieval(#[from] RetrievalError),

    #[error(transparent)]
    Library(#[from] LibraryError),

    #[error(transparent)]
    Ingest(#[from] PdfIngestError),

    #[error(transparent)]
    Insights(#[from] InsightsToolError),

    #[error(transparent)]
    Export(#[from] ExportToolError),

    #[error(transparent)]
    Dataset(#[from] DatasetToolError),

    #[error(transparent)]
    Stop(#[from] StopError),

    #[error(transparent)]
    Watch(#[from] WatchError),
}

impl ToolError {
    /// The text of the failure: the error and every cause under it on one line. A failure that a
    /// spent credit caused names the page where credit is added, and a failure of a store or of an
    /// outside service points at the `health` tool.
    pub(super) fn text(&self) -> String {
        let mut text = error_chain(self)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !text.ends_with(['.', '!', '?']) {
            text.push('.');
        }
        if let Some(credit) = self.spent_credit() {
            text.push(' ');
            text.push_str(&credit);
        }
        if self.is_location_refused() {
            text.push(' ');
            text.push_str(LOCATION_REFUSED_DETAIL);
            text.push('.');
        }
        if self.is_service_failure() {
            text.push(' ');
            text.push_str(HEALTH_HINT);
        }
        text
    }

    /// What to do about a provider that says the prepaid credit is used up, when that is anywhere
    /// in the chain of causes.
    fn spent_credit(&self) -> Option<String> {
        let mut next: Option<&(dyn Error + 'static)> = Some(self);
        while let Some(error) = next {
            if error
                .downcast_ref::<EmbedError>()
                .is_some_and(EmbedError::is_out_of_credit)
            {
                return Some(format!(
                    "Your Google Gemini prepaid credit is used up: add credit at {GEMINI_BILLING_PAGE}, then try again."
                ));
            }
            if error
                .downcast_ref::<JevError>()
                .is_some_and(JevError::is_out_of_credit)
            {
                return Some(format!(
                    "Your Jev credit is used up: add credit in the console at {JEV_CONSOLE_PAGE}, then try again."
                ));
            }
            next = error.source();
        }
        None
    }

    /// True when Google refused the address the server calls from, anywhere in the chain of
    /// causes. The text then says what to do about the address, which is nothing a key can fix.
    fn is_location_refused(&self) -> bool {
        let mut next: Option<&(dyn Error + 'static)> = Some(self);
        while let Some(error) = next {
            if error
                .downcast_ref::<EmbedError>()
                .is_some_and(EmbedError::is_location_refused)
            {
                return true;
            }
            next = error.source();
        }
        false
    }

    fn is_service_failure(&self) -> bool {
        match self {
            ToolError::Retrieval(error) => matches!(
                error,
                RetrievalError::Graph(_)
                    | RetrievalError::Embedder(_)
                    | RetrievalError::ItemStore(_)
                    | RetrievalError::ConceptStore(_)
                    | RetrievalError::Search(_)
                    | RetrievalError::Answer(rag_retrieval::AnswerError::Llm(_))
            ),
            ToolError::Library(error) => error.is_service_failure(),
            ToolError::Insights(error) => is_service_failure_of_insights(error),
            ToolError::Export(error) => match error {
                ExportToolError::Insights(error) => is_service_failure_of_insights(error),
                // None of these is a service: the arguments, the document or the writing of the
                // file is the cause.
                ExportToolError::Format { .. }
                | ExportToolError::NotAbsolute { .. }
                | ExportToolError::Extension { .. }
                | ExportToolError::NoFolder { .. }
                | ExportToolError::Exists { .. }
                | ExportToolError::NotWrittenYet { .. }
                | ExportToolError::BookPart { .. }
                | ExportToolError::Export(_)
                | ExportToolError::Length { .. }
                | ExportToolError::Stopped(_) => false,
            },
            // A conversion also fails for the PDF itself, or for a reply that broke a rule twice,
            // and no service is the cause then. It points at a service only when the services
            // could not start or a call for a page failed.
            ToolError::Ingest(PdfIngestError::Pdf(error)) => match &**error {
                PdfError::Convert(ConvertError::Services(_)) => true,
                PdfError::Convert(ConvertError::PageFailed { source, .. }) => {
                    matches!(**source, PageError::Service(_))
                }
                PdfError::Convert(_) => false,
                PdfError::Graph(_) | PdfError::Store(_) => true,
                PdfError::Ingest(error) => is_service_failure_of_the_ingest(error),
            },
            ToolError::Ingest(error) => matches!(
                error,
                PdfIngestError::Embedder(_)
                    | PdfIngestError::ItemStore(_)
                    | PdfIngestError::ConceptStore(_)
                    | PdfIngestError::Graph(_)
                    | PdfIngestError::ReadMedia(_)
                    | PdfIngestError::Labels(_)
            ),
            ToolError::Dataset(error) => match error {
                DatasetToolError::Graph(_) | DatasetToolError::ModelNotReady(_) => true,
                // A build that the model or the graph stopped points at `health`. A file, a folder
                // or the content folder is not a service, and `health` would call every service
                // ready.
                DatasetToolError::Build(error) => {
                    error.llm_error().is_some() || error.graph_error().is_some()
                }
                // None of these is a service. The text of a job that failed already got its hint,
                // if it needed one, when the job ended.
                DatasetToolError::BlankName
                | DatasetToolError::Name { .. }
                | DatasetToolError::UnknownMedia { .. }
                | DatasetToolError::UnknownDocument { .. }
                | DatasetToolError::UnknownHoldout { .. }
                | DatasetToolError::NothingToExport
                | DatasetToolError::Busy(_)
                | DatasetToolError::SharedBusy(_)
                | DatasetToolError::UnknownJob { .. }
                | DatasetToolError::Lost
                | DatasetToolError::Failed(_)
                | DatasetToolError::Stopped(_) => false,
            },
            ToolError::Stop(StopError::UnknownJob { .. })
            | ToolError::Watch(WatchError::TimeoutOutOfRange { .. }) => false,
        }
    }
}

/// Whether a store or the agent stopped a tool that reads or writes insights.
fn is_service_failure_of_insights(error: &InsightsToolError) -> bool {
    match error {
        InsightsToolError::Library(error) => error.is_service_failure(),
        InsightsToolError::Graph(_)
        | InsightsToolError::Category(_)
        | InsightsToolError::ModelNotReady(_) => true,
        // None of these is a service. The text of a job that failed already got its hint, if it
        // needed one, when the job ended.
        InsightsToolError::Unreadable(_)
        | InsightsToolError::Choice
        | InsightsToolError::Busy(_)
        | InsightsToolError::SharedBusy(_)
        | InsightsToolError::UnknownJob { .. }
        | InsightsToolError::Lost
        | InsightsToolError::Failed(_)
        | InsightsToolError::Stopped(_) => false,
    }
}

/// Whether a store or an outside service stopped the ingest of a converted chapter. A file that
/// cannot be read or written, a reply that the model got wrong twice and stores that do not belong
/// together are not that, and `health` would call every service ready.
fn is_service_failure_of_the_ingest(error: &IngestError) -> bool {
    match error {
        IngestError::ModelNotReady(_)
        | IngestError::Embed(_)
        | IngestError::Store(_)
        | IngestError::Graph(_) => true,
        IngestError::Concepts(error) => matches!(
            error,
            ConceptError::Stopped { .. }
                | ConceptError::Embed { .. }
                | ConceptError::ConceptStore(_)
                | ConceptError::RelatedItems { .. }
                | ConceptError::Graph(_)
        ),
        _ => false,
    }
}

/// A tool error is a result with `isError` set and a text, never a protocol error: a client shows
/// a protocol error without its message.
impl IntoCallToolResult for ToolError {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        Ok(CallToolResult::error(vec![ContentBlock::text(self.text())]).into())
    }
}
