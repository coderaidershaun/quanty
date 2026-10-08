//! The `ingest_pdf` and `ingest_status` tools. An ingest takes minutes and a client puts a time
//! limit on a tool call, so `ingest_pdf` starts a job and `ingest_status` reads it.

mod error;
mod jobs;
mod pdf;
mod work;

use std::sync::Arc;

use graph::FalkorGraph;
use rag_core::{Category, Config};
use rag_ingestion::media_category;
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
    /// The title of the media the PDF belongs to, such as "Option Volatility and Pricing".
    // An agent that still sends the older name `book` is understood. The schema shows `media`
    // only.
    #[serde(alias = "book")]
    media: String,
    /// `book` (the default), `paper` or `other`. A book's PDF must be named
    /// `chapter-<number>-<name>.pdf`; a paper or other PDF can have any name. Used only when the
    /// library does not have this media yet: a stored media keeps its own category, which says
    /// how the PDF is named.
    category: Option<String>,
    /// The authors of the media. Used only when the library does not have this media yet.
    authors: Option<Vec<String>>,
    /// Tags of the media, such as "options". Used only when the library does not have this media
    /// yet.
    tags: Option<Vec<String>>,
    /// Tags of this PDF only.
    document_tags: Option<Vec<String>>,
    /// The title of this document, for a paper or other media. It defaults to the title of the
    /// media. Not for a book, whose chapter is named by its file name. Whether the media is a
    /// book is up to the library when it has this media already: a stored media keeps its own
    /// category, which says how the PDF is named.
    document_title: Option<String>,
    /// The absolute path of the PDF on the machine the server runs on. A book's file must be
    /// named `chapter-<number>-<name>.pdf`.
    path: Option<String>,
    /// The bytes of the PDF as standard base64, for a client that cannot reach the disk of the
    /// server. It goes with `file_name`.
    pdf_base64: Option<String>,
    /// The name to save the bytes of `pdf_base64` under, with no folder in it; for a book it must
    /// be `chapter-<number>-<name>.pdf`. Not for `path`.
    file_name: Option<String>,
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
        // A store that is down is reported like a failed job, because sending the same PDF again
        // goes on from there.
        let category = self
            .category_of(&checked)
            .await
            .map_err(|error| PdfIngestError::Failed(format!("{} {GO_ON}", error_text(error))))?;
        let ready = checked.named(category, &self.config)?;
        let started = self.jobs.start(&ready.media, &ready.file_name)?;
        self.spawn(ready, started.report, error_text);
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

    /// The category that names the PDF: the one the library has for its media, else the one the
    /// agent gave, else a book.
    async fn category_of(&self, checked: &pdf::CheckedPdf) -> Result<Category, PdfIngestError> {
        let graph = FalkorGraph::connect(&self.config).await?;
        media_category(&checked.media, checked.category, &graph)
            .await
            .map_err(PdfIngestError::ReadMedia)
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
        ready: pdf::ReadyPdf,
        report: tokio::sync::watch::Sender<IngestReport>,
        error_text: fn(PdfIngestError) -> String,
    ) {
        let work = Work {
            services: Arc::clone(&self.services),
            config: self.config.clone(),
            pdf: ready,
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
