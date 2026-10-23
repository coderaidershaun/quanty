//! The `build_dataset` and `dataset_status` tools. A build takes minutes to hours and a client puts
//! a time limit on a tool call, so `build_dataset` starts a job and `dataset_status` reads it.

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
use crate::jobs::{BackgroundJobs, BackgroundReservation, JobKind, Jobs, StopState};
use crate::services::Services;
pub(crate) use error::DatasetToolError;
pub(crate) use report::DatasetReport;
use report::DatasetState;
use work::Work;

/// What an agent gives `build_dataset`. Only `name` is needed: every other argument has the
/// default that `rag-dataset build` has.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct BuildDatasetArgs {
    /// The name of the dataset: letters, digits, `-`, `_` and `.`. It is the name of its folder in
    /// the dataset folder. A build with the name of an earlier one replaces that dataset's files.
    name: String,
    /// Only the documents of these media, by title whatever the capitals. Every media when left
    /// out.
    media: Option<Vec<String>>,
    /// Only these documents, each a `document_id` or the whole `title` from `list_documents`.
    /// Every document of the chosen media when left out.
    documents: Option<Vec<String>>,
    /// Documents kept out of training for evaluation, named like `documents`. Their sections go to
    /// `cpt/heldout.jsonl`, and no synthetic text is written from them.
    holdout: Option<Vec<String>>,
    /// Also export the parts of a book that are not chapters, such as its index. An appendix is
    /// always exported. False when left out.
    include_book_parts: Option<bool>,
    /// How many rewritten texts are written from each passage of a training section, a part of it
    /// of at most 2,500 estimated tokens. 3 when left out.
    rewrites: Option<u32>,
    /// How many questions each section gets. 2 when left out.
    questions: Option<u32>,
    /// The most fine-tuning examples the build writes. 300 when left out.
    max_sft: Option<u32>,
    /// The most synthetic texts the build writes. No limit when left out.
    max_synthetic: Option<u32>,
    /// How many times the model answers each question, to check that its answers agree. 3 when
    /// left out.
    samples: Option<u32>,
    /// Run the Python that the model writes for an answer, on the machine the server runs on, to
    /// check it. True when left out.
    run_code: Option<bool>,
    /// Export, check and plan only. It asks no model and costs nothing, and its `summary` gives
    /// the texts, questions and model calls that the full build would make. False when left out.
    dry_run: Option<bool>,
    /// A section whose worst page matches less of the PDF's own text than this share, from 0 to
    /// 1, is rejected. 0.90 when left out.
    min_page_score: Option<f64>,
    /// Keep a section whose formulas do not render, instead of rejecting it. False when left out.
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
    /// The `job_id` that `build_dataset` gave.
    job_id: String,
}

/// The dataset builds of one server, and what they need to run.
pub(crate) struct Datasets<S> {
    config: Config,
    services: Arc<S>,
    jobs: Jobs<DatasetReport>,
    background_jobs: Option<BackgroundJobs>,
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
            jobs: Jobs::default(),
            background_jobs,
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
        let reservation: Option<BackgroundReservation> = self
            .background_jobs
            .as_ref()
            .map(|jobs| jobs.reserve(JobKind::Dataset, args.name.clone()))
            .transpose()?;
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
        let mut watcher = started.watcher;
        let report = watcher
            .wait_for(|report| report.stage.is_some() || report.state != DatasetState::Running)
            .await
            .map(|report| report.clone())
            .map_err(|_| DatasetToolError::Lost)?;
        match report.state {
            DatasetState::Failed => Err(DatasetToolError::Failed(report.error.unwrap_or_default())),
            DatasetState::Running | DatasetState::Done | DatasetState::Stopped => Ok(report),
        }
    }

    /// Stops the build with this id when it is running. `None` when no build has the id.
    pub(crate) async fn stop(&self, job_id: &str) -> Option<StopState> {
        self.jobs.stop(job_id).await
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
