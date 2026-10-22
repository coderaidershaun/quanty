//! The `ingest_pdf` and `ingest_status` tools. An ingest takes minutes and a client puts a time
//! limit on a tool call, so `ingest_pdf` starts a job and `ingest_status` reads it.

mod error;
mod pdf;
mod report;
mod work;

use std::sync::Arc;

use graph::FalkorGraph;
use rag_core::Config;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::jobs::{Jobs, StopState};
use crate::services::Services;
pub(crate) use error::PdfIngestError;
pub(crate) use pdf::base64_len;
pub(crate) use report::IngestReport;
use report::JobState;
use work::Work;

/// What an agent gives `ingest_pdf`. Give exactly one of `path` and `pdf_base64`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct IngestPdfArgs {
    /// The title of the media the PDF belongs to, such as "Option Volatility and Pricing".
    // An agent that still sends the older name `book` is understood. The schema shows `media`
    // only.
    #[serde(alias = "book")]
    media: String,
    /// `book` (the default), `paper` or `other`. A book's chapter must be named
    /// `chapter-<number>-<name>.pdf`; a part of a book that is not a chapter takes
    /// `document_title` and can have any name; a paper or other PDF can have any name. Used only
    /// when the library does not have this media yet: a stored media keeps its own category,
    /// which says how the PDF is named.
    category: Option<String>,
    /// The authors of the media. Used only when the library does not have this media yet.
    authors: Option<Vec<String>>,
    /// Tags of the media, such as "options". Used only when the library does not have this media
    /// yet.
    tags: Option<Vec<String>>,
    /// Tags of this PDF only.
    document_tags: Option<Vec<String>>,
    /// The title of this document. For a paper or other media it defaults to the title of the
    /// media. For a book it names a part that is not a chapter, such as "Index" or "Glossary":
    /// the document is stored as "<book>, <title>" and the file can have any name; leave it out
    /// for a book's chapter, whose file is named `chapter-<number>-<name>.pdf`. Whether the
    /// media is a book is up to the library when it has this media already: a stored media keeps
    /// its own category, which says how the PDF is named.
    document_title: Option<String>,
    /// The absolute path of the PDF on the machine the server runs on. A book's chapter file
    /// must be named `chapter-<number>-<name>.pdf`; a part that is not a chapter takes
    /// `document_title` and any file name.
    path: Option<String>,
    /// The bytes of the PDF as standard base64, for a client that cannot reach the disk of the
    /// server. It goes with `file_name`.
    pdf_base64: Option<String>,
    /// The name to save the bytes of `pdf_base64` under, with no folder in it; for a book's
    /// chapter it must be `chapter-<number>-<name>.pdf`, and a part that is not a chapter takes
    /// `document_title` and any name. Not for `path`.
    file_name: Option<String>,
}

/// What an agent gives `ingest_status`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct IngestStatusArgs {
    /// The `job_id` that `ingest_pdf` gave.
    job_id: String,
}

/// The end of the text of a job that failed or that `stop_job` stopped.
const GO_ON: &str =
    "Send the same PDF again to go on: converted pages and kept answers are not paid for twice.";

/// The ingest jobs of one server, and what they need to run.
pub(crate) struct Ingest<S> {
    config: Config,
    services: Arc<S>,
    max_pdf_bytes: u64,
    jobs: Jobs<IngestReport>,
}

impl<S: Services> Ingest<S> {
    pub(crate) fn new(config: Config, services: Arc<S>, max_pdf_bytes: u64) -> Ingest<S> {
        Ingest {
            config,
            services,
            max_pdf_bytes,
            jobs: Jobs::default(),
        }
    }

    /// Checks the PDF, starts a job and answers once the paid work has begun, or the job ended
    /// before it could. `error_text` turns the error of a job into the text that the agent
    /// reads. The server gives it, because the server words every tool error in one place.
    ///
    /// # Errors
    /// A PDF that is refused, a graph that cannot say which media the library has, an ingest that
    /// is already running, or a job that failed before the paid work began.
    pub(crate) async fn start(
        &self,
        args: IngestPdfArgs,
        error_text: fn(PdfIngestError) -> String,
    ) -> Result<IngestReport, PdfIngestError> {
        let config = self.config.clone();
        let max_pdf_bytes = self.max_pdf_bytes;
        // The check decodes a PDF that was sent as base64, which can be megabytes of text, so it
        // runs on a thread that may block and the other calls are not held up.
        let checked = tokio::task::spawn_blocking(move || pdf::check(args, &config, max_pdf_bytes))
            .await
            .map_err(PdfIngestError::Stopped)??;
        let (ready, graph) = self.named(checked, error_text).await?;
        let started = self
            .jobs
            .start(|id| IngestReport::running(id, &ready.media, &ready.file_name))?;
        let report = started.report.clone();
        let work = Work {
            services: Arc::clone(&self.services),
            config: self.config.clone(),
            pdf: ready,
            graph,
            report: started.report,
        };
        self.jobs.spawn(
            started.id,
            report,
            work.run(),
            move |report, ended| match ended {
                Ok(outcome) => report.finish(&outcome),
                Err(error) => report.fail(format!("{} {GO_ON}", error_text(error))),
            },
        );
        let mut watcher = started.watcher;
        let report = watcher
            .wait_for(|report| report.stage.is_some() || report.state != JobState::Running)
            .await
            .map(|report| report.clone())
            .map_err(|_| PdfIngestError::Lost)?;
        match report.state {
            JobState::Failed => Err(PdfIngestError::Failed(report.error.unwrap_or_default())),
            JobState::Running | JobState::Done | JobState::AlreadyIngested | JobState::Stopped => {
                Ok(report)
            }
        }
    }

    /// The PDF named by the category that the library has for its media, and the graph that the
    /// job goes on to write to. A store that is down is reported like a failed job, because
    /// sending the same PDF again goes on from there.
    async fn named(
        &self,
        checked: pdf::CheckedPdf,
        error_text: fn(PdfIngestError) -> String,
    ) -> Result<(pdf::ReadyPdf, FalkorGraph), PdfIngestError> {
        let store_down = |error: PdfIngestError| {
            PdfIngestError::Failed(format!("{} {GO_ON}", error_text(error)))
        };
        let graph = FalkorGraph::connect(&self.config)
            .await
            .map_err(|error| store_down(error.into()))?;
        let ready = checked
            .named(&graph, &self.config)
            .await
            .map_err(|error| match error {
                PdfIngestError::ReadMedia(_) => store_down(error),
                refused => refused,
            })?;
        Ok((ready, graph))
    }

    /// Stops the job with this id when it is running. `None` when no ingest job has the id.
    pub(crate) async fn stop(&self, job_id: &str) -> Option<StopState> {
        self.jobs.stop(job_id).await
    }

    /// The latest report of a job.
    ///
    /// # Errors
    /// [`PdfIngestError::UnknownJob`] when no job has the id.
    pub(crate) fn status(&self, args: IngestStatusArgs) -> Result<IngestReport, PdfIngestError> {
        self.jobs
            .report_of(&args.job_id)
            .ok_or(PdfIngestError::UnknownJob {
                job_id: args.job_id,
            })
    }
}
