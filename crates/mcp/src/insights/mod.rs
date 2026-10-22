//! The `chapter_insights`, `write_insights` and `insights_status` tools. Writing the insights of
//! stored documents takes a minute or more for each, and a client puts a time limit on a tool
//! call, so `write_insights` starts a job and `insights_status` reads it.

mod error;
mod report;
mod saved;
mod work;

use std::sync::Arc;

use graph::FalkorGraph;
use rag_core::{Config, DocId, Llm, WorkKind};
use rag_ingestion::{ChapterChoice, InsightsNeed, InsightsWriter, StoredChapter, stored_chapters};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::task::JoinError;

use crate::args::non_blank;
use crate::jobs::{Jobs, StopState};
use crate::library::{LibraryError, parse_document_id};
use crate::services::Services;
pub(crate) use error::InsightsToolError;
pub(crate) use report::InsightsReport;
use report::InsightsState;
pub(crate) use saved::{
    ChapterInsightsArgs, InsightsView, Saved, chapter_insights, saved_insights,
};
use work::Work;

/// What an agent gives `write_insights`. Give exactly one of `document_id`, `media` and `all`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct WriteInsightsArgs {
    /// One stored document: its `document_id` from `list_documents`.
    document_id: Option<String>,
    /// Every document of this media, by title whatever the capitals.
    media: Option<String>,
    /// True for every stored document that belongs to a media.
    all: Option<bool>,
    /// List what would be written and ask nothing. It costs nothing. False when left out.
    dry_run: Option<bool>,
}

/// What an agent gives `insights_status`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct InsightsStatusArgs {
    /// The `job_id` that `write_insights` gave.
    job_id: String,
}

/// The documents an agent chose to write the insights of.
enum Chosen {
    Document(DocId),
    Media(String),
    All,
}

impl Chosen {
    fn as_choice(&self) -> ChapterChoice<'_> {
        match self {
            Chosen::Document(id) => ChapterChoice::Document(*id),
            Chosen::Media(title) => ChapterChoice::Media(title),
            Chosen::All => ChapterChoice::All,
        }
    }
}

impl WriteInsightsArgs {
    /// # Errors
    /// [`InsightsToolError::Choice`] unless exactly one of the three is given, and the error of a
    /// `document_id` that is not valid.
    fn chosen(&self) -> Result<Chosen, InsightsToolError> {
        let document = non_blank(self.document_id.clone());
        let media = non_blank(self.media.clone());
        let all = self.all == Some(true);
        match (document, media, all) {
            (Some(document), None, false) => Ok(Chosen::Document(parse_document_id(&document)?)),
            (None, Some(media), false) => Ok(Chosen::Media(media)),
            (None, None, true) => Ok(Chosen::All),
            _ => Err(InsightsToolError::Choice),
        }
    }
}

/// The insights jobs of one server, and what they need to run.
pub(crate) struct Insights<S> {
    config: Config,
    services: Arc<S>,
    jobs: Jobs<InsightsReport>,
}

impl<S: Services> Insights<S> {
    pub(crate) fn new(config: Config, services: Arc<S>) -> Insights<S> {
        Insights {
            config,
            services,
            jobs: Jobs::default(),
        }
    }

    /// Starts a job and answers once its first question is out or it ended. `error_text` turns
    /// the error of a job into the text that the agent reads. The server gives it, because the
    /// server words every tool error in one place.
    ///
    /// # Errors
    /// A choice that is not valid, a graph or a content folder that cannot be read, a document or
    /// media that is not stored, an agent that cannot be asked (not for a dry run), a job that is
    /// already running, or a job that failed before its first question.
    pub(crate) async fn start(
        &self,
        args: WriteInsightsArgs,
        error_text: fn(InsightsToolError) -> String,
    ) -> Result<InsightsReport, InsightsToolError> {
        let chosen = args.chosen()?;
        let dry_run = args.dry_run == Some(true);
        let graph = FalkorGraph::connect(&self.config).await?;
        let chapters = stored_chapters(chosen.as_choice(), &graph, &self.config.content_folder)
            .await
            .map_err(LibraryError::from)?;
        let writer = self.writer_for(&chapters, dry_run).await?;
        let started = self
            .jobs
            .start(|id| InsightsReport::running(id, dry_run, chapters.len()))?;
        let report = started.report.clone();
        let work = Work {
            chapters,
            writer,
            content_folder: self.config.content_folder.clone(),
            report: started.report,
        };
        self.jobs.spawn(
            started.id,
            report,
            async move {
                work.run().await;
                Ok::<(), JoinError>(())
            },
            move |report, ended| match ended {
                Ok(()) => report.finish(),
                Err(panicked) => report.fail(error_text(InsightsToolError::Stopped(panicked))),
            },
        );
        let mut watcher = started.watcher;
        let report = watcher
            .wait_for(|report| report.current.is_some() || report.state != InsightsState::Running)
            .await
            .map(|report| report.clone())
            .map_err(|_| InsightsToolError::Lost)?;
        match report.state {
            InsightsState::Failed => {
                Err(InsightsToolError::Failed(report.error.unwrap_or_default()))
            }
            InsightsState::Running | InsightsState::Done | InsightsState::Stopped => Ok(report),
        }
    }

    /// The writer of a job that asks the model, checked once so that an agent that is not signed
    /// in fails the call before the first document. That check costs nothing, and a job with
    /// nothing to write never starts the agent.
    async fn writer_for(
        &self,
        chapters: &[StoredChapter],
        dry_run: bool,
    ) -> Result<Option<InsightsWriter<S::Llm>>, InsightsToolError> {
        let has_work = chapters
            .iter()
            .any(|chapter| matches!(chapter.need(), InsightsNeed::Write(_)));
        if dry_run || !has_work {
            return Ok(None);
        }
        let llm = self.services.llm(WorkKind::Ingest);
        llm.check_ready()
            .await
            .map_err(InsightsToolError::ModelNotReady)?;
        Ok(Some(InsightsWriter::new(llm)))
    }

    /// Stops the job with this id when it is running. `None` when no insights job has the id.
    pub(crate) async fn stop(&self, job_id: &str) -> Option<StopState> {
        self.jobs.stop(job_id).await
    }

    /// The latest report of a job.
    ///
    /// # Errors
    /// [`InsightsToolError::UnknownJob`] when no job has the id.
    pub(crate) fn status(
        &self,
        args: InsightsStatusArgs,
    ) -> Result<InsightsReport, InsightsToolError> {
        self.jobs
            .report_of(&args.job_id)
            .ok_or(InsightsToolError::UnknownJob {
                job_id: args.job_id,
            })
    }
}
