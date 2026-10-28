//! The `build_dataset` and `dataset_status` tools. A build takes minutes to hours and a client puts
//! a time limit on a tool call, so `build_dataset` starts a job and `job_status` reads it;
//! `dataset_status` is the older name for the same read.

mod error;
mod report;
mod work;

use std::sync::Arc;

use graph::FalkorGraph;
use rag_core::{Config, Llm, WorkKind};
use rag_dataset::{BuildOptions, Selection};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::args::non_blank;
use crate::jobs::{BackgroundJobs, JobKind, JobReport, JobState};
use crate::jobs::{Jobs, first_report};
use crate::services::Services;
pub(crate) use error::DatasetToolError;
pub(crate) use report::DatasetReport;
use work::Work;

/// What an agent gives `build_dataset`. Only `name` is needed: every other argument has the
/// default that `rag-dataset build` has.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct BuildDatasetArgs {
    /// Dataset and folder name.
    name: String,
    /// Only these media.
    media: Option<Vec<String>>,
    /// Only these documents.
    documents: Option<Vec<String>>,
    /// Documents kept for evaluation.
    holdout: Option<Vec<String>>,
    /// Also export non-chapter parts.
    include_book_parts: Option<bool>,
    /// Rewrites per passage. Default 3.
    rewrites: Option<u32>,
    /// Questions per section. Default 2.
    questions: Option<u32>,
    /// Most fine-tuning examples.
    max_sft: Option<u32>,
    /// Most synthetic texts.
    max_synthetic: Option<u32>,
    /// Answers per question. Default 3.
    samples: Option<u32>,
    /// Run model code to check it.
    run_code: Option<bool>,
    /// Plan only; asks no model.
    dry_run: Option<bool>,
    /// Section page match, 0 to 1.
    min_page_score: Option<f64>,
    /// Keep sections that do not render.
    keep_unrendered: Option<bool>,
}

impl BuildDatasetArgs {
    /// # Errors
    /// [`DatasetToolError::BlankName`] when `name` is blank.
    fn options(self, config: &Config) -> Result<BuildOptions, DatasetToolError> {
        let name = non_blank(Some(self.name)).ok_or(DatasetToolError::BlankName)?;
        let mut options = BuildOptions::new(&name, config);
        options.selection = Selection {
            media: entries(self.media),
            documents: entries(self.documents),
            holdout: entries(self.holdout),
        };
        options.include_book_parts = self
            .include_book_parts
            .unwrap_or(options.include_book_parts);
        options.rewrites = self.rewrites.unwrap_or(options.rewrites);
        options.questions = self.questions.unwrap_or(options.questions);
        options.max_sft = self.max_sft.unwrap_or(options.max_sft);
        options.max_synthetic = self.max_synthetic.or(options.max_synthetic);
        options.samples = self.samples.unwrap_or(options.samples);
        options.run_code = self.run_code.unwrap_or(options.run_code);
        options.generation = !self.dry_run.unwrap_or(false);
        options.min_page_score = self.min_page_score.unwrap_or(options.min_page_score);
        options.keep_unrendered = self.keep_unrendered.unwrap_or(options.keep_unrendered);
        Ok(options)
    }
}

/// Trims each entry and leaves out the blank ones.
fn entries(texts: Option<Vec<String>>) -> Vec<String> {
    texts
        .unwrap_or_default()
        .into_iter()
        .filter_map(|text| non_blank(Some(text)))
        .collect()
}

/// What an agent gives `dataset_status`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct DatasetStatusArgs {
    /// The `job_id` of a job.
    job_id: String,
}

/// The dataset builds of one server, and what they need to run.
pub(crate) struct Datasets<S> {
    config: Config,
    services: Arc<S>,
    pub(crate) jobs: Jobs<DatasetReport>,
}

impl<S: Services> Datasets<S> {
    pub(crate) fn new(
        config: Config,
        services: Arc<S>,
        background_jobs: Option<BackgroundJobs>,
    ) -> Datasets<S> {
        Datasets {
            config,
            services,
            jobs: Jobs::new(JobKind::Dataset, background_jobs),
        }
    }

    /// Starts a build and answers once its first step arrived or it ended. `error_text` turns the
    /// error of a job into the text that the agent reads. The server gives it, because the server
    /// words every tool error in one place.
    ///
    /// # Errors
    /// A blank name, a graph that cannot be reached, a model that is not ready (not for a dry
    /// run), a build that is already running, or a build that failed before its first step.
    pub(crate) async fn start(
        &self,
        args: BuildDatasetArgs,
        error_text: fn(DatasetToolError) -> String,
    ) -> Result<DatasetReport, DatasetToolError> {
        let reservation = self.jobs.reserve(|| args.name.clone())?;
        let options = args.options(&self.config)?;
        let graph = FalkorGraph::connect(&self.config).await?;
        let llm = self.services.llm(WorkKind::Datasets);
        if options.generation {
            llm.check_ready()
                .await
                .map_err(DatasetToolError::ModelNotReady)?;
        }
        let started = self
            .jobs
            .start(|id| DatasetReport::running(id, &options.out))?;
        let report = started.report.clone();
        let work = Work {
            options,
            llm,
            graph,
            report: started.report,
        };
        self.jobs.spawn(
            started.id,
            report,
            work.run(),
            move |report, ended| match ended {
                Ok(built) => report.finish(&built),
                Err(error) => report.fail(error_text(error)),
            },
            reservation,
        );
        let report = first_report(started.watcher, |report| {
            report.job().stage().is_some() || !report.is_running()
        })
        .await
        .ok_or(DatasetToolError::Lost)?;
        match report.job().state() {
            JobState::Failed => Err(DatasetToolError::Failed(
                report.job().error().unwrap_or_default().to_owned(),
            )),
            JobState::Running | JobState::Done | JobState::AlreadyIngested | JobState::Stopped => {
                Ok(report)
            }
        }
    }

    /// The latest report of a build.
    ///
    /// # Errors
    /// [`DatasetToolError::UnknownJob`] when no job has the id.
    pub(crate) fn status(
        &self,
        args: DatasetStatusArgs,
    ) -> Result<DatasetReport, DatasetToolError> {
        self.jobs
            .report_of(&args.job_id)
            .ok_or(DatasetToolError::UnknownJob {
                job_id: args.job_id,
            })
    }
}
