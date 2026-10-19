//! The work of one build job: one call of `rag_dataset::build`, whose steps are written into the
//! job's report as they come. It needs the model and the graph and nothing else of the services.

use graph::FalkorGraph;
use rag_core::{Llm, UsageTally};
use rag_dataset::{BuildOptions, Step};
use tokio::sync::watch;

use super::DatasetToolError;
use super::report::DatasetReport;

/// One build job, with everything it needs to run on a task of its own.
pub(super) struct Work<L> {
    pub(super) options: BuildOptions,
    pub(super) llm: L,
    /// The graph that the plan reads the concepts from, connected before the job started.
    pub(super) graph: FalkorGraph,
    /// Where each step is written. The end of the report is written by the task that waits for
    /// this one, not here.
    pub(super) report: watch::Sender<DatasetReport>,
}

impl<L: Llm + Send + Sync + 'static> Work<L> {
    pub(super) async fn run(self) -> Result<rag_dataset::Report, DatasetToolError> {
        let Work {
            options,
            llm,
            graph,
            report,
        } = self;
        let mut on_step = |step: Step, usage: &UsageTally| {
            report.send_modify(|report| report.progress(step, usage));
        };
        Ok(rag_dataset::build(&options, &llm, Some(&graph), &mut on_step).await?)
    }
}
