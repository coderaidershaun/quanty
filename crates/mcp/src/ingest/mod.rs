//! The `ingest_pdf` and `ingest_status` tools, and the `check_pdf` tool that says first what an
//! ingest would do. An ingest takes minutes and a client puts a time limit on a tool call, so
//! `ingest_pdf` starts a job and `job_status` reads it; `ingest_status` is the older name for
//! the same read.

mod check;
mod error;
mod fetch;
mod pdf;
mod progress;
mod report;
mod work;

use std::sync::Arc;

use graph::FalkorGraph;
use rag_core::Config;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::args::file_name_of;
use crate::jobs::{BackgroundJobs, JobKind, JobReport, JobState};
use crate::jobs::{Jobs, first_report};
use crate::services::Services;
pub(crate) use check::{CheckPdfArgs, PdfCheck};
pub(crate) use error::PdfIngestError;
pub(crate) use pdf::base64_len;
pub(crate) use report::IngestReport;
use work::Work;

const PDF_START: &[u8] = b"%PDF-";

/// What an agent gives `ingest_pdf`. Give exactly one of `path`, `pdf_base64` and `pdf_url`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct IngestPdfArgs {
    // An agent that still sends the older name `book` is understood. The schema shows `media`
    // only.
    /// Media title.
    #[serde(alias = "book")]
    media: String,
    /// `book`, `paper` or `other`.
    category: Option<String>,
    /// Used when the media is new.
    authors: Option<Vec<String>>,
    /// Used when the media is new.
    tags: Option<Vec<String>>,
    /// Tags of this PDF only.
    document_tags: Option<Vec<String>>,
    /// Title of this document.
    document_title: Option<String>,
    /// Absolute PDF path on the server.
    path: Option<String>,
    /// PDF bytes as base64.
    pdf_base64: Option<String>,
    /// URL to fetch the PDF from.
    pdf_url: Option<String>,
    /// Name for the saved PDF.
    file_name: Option<String>,
}

/// What an agent gives `ingest_status`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct IngestStatusArgs {
    /// The `job_id` of a job.
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
    pub(crate) jobs: Jobs<IngestReport>,
}

impl<S: Services> Ingest<S> {
    pub(crate) fn new(
        config: Config,
        services: Arc<S>,
        max_pdf_bytes: u64,
        background_jobs: Option<BackgroundJobs>,
    ) -> Ingest<S> {
        Ingest {
            config,
            services,
            max_pdf_bytes,
            jobs: Jobs::new(JobKind::Ingest, background_jobs),
        }
    }

    /// Checks the PDF, starts a job and answers once the paid work has begun (the job is past
    /// checking the stores), or the job ended before it could. `error_text` turns the error of a
    /// job into the text that the agent reads. The server gives it, because the server words every
    /// tool error in one place.
    ///
    /// # Errors
    /// A PDF that is refused, a graph that cannot say which media the library has, an ingest that
    /// is already running, or a job that failed before the paid work began.
    pub(crate) async fn start(
        &self,
        args: IngestPdfArgs,
        error_text: fn(PdfIngestError) -> String,
    ) -> Result<IngestReport, PdfIngestError> {
        let reservation = self.jobs.reserve(|| {
            args.file_name
                .clone()
                .or_else(|| args.path.as_deref().and_then(file_name_of))
                .unwrap_or_else(|| args.media.clone())
        })?;
        let checked = pdf::check(args, &self.config, self.max_pdf_bytes).await?;
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
            reservation,
        );
        let report = first_report(started.watcher, IngestReport::is_ready_to_answer)
            .await
            .ok_or(PdfIngestError::Lost)?;
        match report.job().state() {
            JobState::Failed => Err(PdfIngestError::Failed(
                report.job().error().unwrap_or_default().to_owned(),
            )),
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
