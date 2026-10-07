//! The `ingest_pdf` and `ingest_status` tools. An ingest takes minutes and a client puts a time
//! limit on a tool call, so `ingest_pdf` starts a job and `ingest_status` reads it. The call to
//! `ingest_pdf` answers once the paid work has begun, or at once when it cannot begin.

mod error;
mod jobs;
mod pdf;
mod work;

use std::sync::Arc;

use rag_core::Config;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::services::Services;
pub(crate) use error::PdfIngestError;
pub(crate) use jobs::IngestReport;
use jobs::{JobState, Jobs};
pub(crate) use pdf::base64_len;
use work::Work;

/// What an agent gives `ingest_pdf`. Give exactly one of `path` and `pdf_base64`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct IngestPdfArgs {
    /// The title of the book the chapter is from, such as "Option Volatility and Pricing".
    book: String,
    /// The absolute path of the chapter PDF on the machine the server runs on. The file must be
    /// named `chapter-<number>-<name>.pdf`.
    path: Option<String>,
    /// The bytes of the chapter PDF as standard base64, for a client that cannot reach the disk of
    /// the server. It goes with `file_name`.
    pdf_base64: Option<String>,
    /// The name to save the bytes of `pdf_base64` under: `chapter-<number>-<name>.pdf`, with no
    /// folder in it. Not for `path`.
    file_name: Option<String>,
    /// The author of the book. It replaces the author that the document has.
    author: Option<String>,
    /// Tags to add to the document, such as "options".
    tags: Option<Vec<String>>,
}

/// What an agent gives `ingest_status`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct IngestStatusArgs {
    /// The `job_id` that `ingest_pdf` gave.
    job_id: String,
}

/// The end of the text of a job that failed.
const GO_ON: &str =
    "Send the same PDF again to go on: converted pages and kept answers are not paid for twice.";

/// The ingest jobs of one server, and what they need to run.
pub(crate) struct Ingest<S> {
    config: Config,
    services: Arc<S>,
    max_pdf_bytes: u64,
    jobs: Jobs,
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
    /// reads, so that the one place that words a tool error is not here.
    ///
    /// # Errors
    /// A PDF that is refused, an ingest that is already running, or a job that failed before
    /// the paid work began.
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
        let started = self.jobs.start(&checked.book, &checked.file_name)?;
        self.spawn(checked, started.report, error_text);
        let mut watcher = started.watcher;
        let report = watcher
            .wait_for(|report| report.stage.is_some() || report.state != JobState::Running)
            .await
            .map(|report| report.clone())
            .map_err(|_| PdfIngestError::Lost)?;
        match report.state {
            JobState::Failed => Err(PdfIngestError::Failed(report.error.unwrap_or_default())),
            JobState::Running | JobState::Done | JobState::AlreadyIngested => Ok(report),
        }
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

    /// Starts the two tasks of a job. The work task goes on after the tool call ends. The task
    /// that waits for it writes the end of the report, and it must not panic: without that end
    /// the report of the job would say `running` for ever.
    fn spawn(
        &self,
        checked: pdf::CheckedPdf,
        report: tokio::sync::watch::Sender<IngestReport>,
        error_text: fn(PdfIngestError) -> String,
    ) {
        let work = Work {
            services: Arc::clone(&self.services),
            config: self.config.clone(),
            pdf: checked,
            report: report.clone(),
        };
        let running = tokio::spawn(async move { work.run().await });
        tokio::spawn(async move {
            let ended = match running.await {
                Ok(ended) => ended,
                Err(stopped) => Err(PdfIngestError::Stopped(stopped)),
            };
            match ended {
                Ok(outcome) => report.send_modify(|report| report.finish(&outcome)),
                Err(error) => {
                    let text = format!("{} {GO_ON}", error_text(error));
                    report.send_modify(|report| report.fail(text));
                }
            }
        });
    }
}
