//! Builds a training dataset from the converted documents, as the command `rag-dataset build`
//! does, and tells the window each step with what the model has cost so far.

use rag_core::Config;
use rag_dataset::{BuildOptions, Report};

use super::{LiveContext, Services};
use crate::backend::Reply;
use crate::contract::{DatasetProgress, DatasetReport, DatasetRequest, Event, Failure, RequestId};

/// A cancel aborts the task. Exactly one `DatasetFinished` ends a build that is not aborted.
pub(super) async fn build<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    wanted: &DatasetRequest,
    reply: &Reply,
) {
    let result = finish(cx, request, wanted, reply).await;
    reply.send(Event::DatasetFinished { request, result });
}

async fn finish<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    wanted: &DatasetRequest,
    reply: &Reply,
) -> Result<DatasetReport, Failure> {
    // A dry run plans too, and the plan takes the graph, so every build connects first and a
    // graph that is down fails it.
    let graph = cx.graph().await?;
    let llm = cx.dataset_llm();
    let config = cx.config();
    let options = options_of(wanted, &config);
    let report = rag_dataset::build(&options, &llm, Some(&graph), &mut |step, spent| {
        let progress = DatasetProgress {
            stage: step.into(),
            spent: spent.into(),
        };
        reply.send(Event::DatasetProgress { request, progress });
    })
    .await
    .map_err(|error| cx.failure(error))?;
    Ok(report_of(report))
}

fn options_of(wanted: &DatasetRequest, config: &Config) -> BuildOptions {
    let mut options = BuildOptions::new(&wanted.name, config);
    options.selection.media = wanted.media.iter().cloned().collect();
    options.selection.holdout = wanted
        .holdout
        .iter()
        .map(|id| rag_core::DocId::from(*id).to_string())
        .collect();
    options.rewrites = wanted.rewrites;
    options.questions = wanted.questions;
    options.max_sft = wanted.max_sft;
    options.generation = !wanted.dry_run;
    options
}

fn report_of(report: Report) -> DatasetReport {
    // SMELL: the dataset crate counts the examples by the words its files use and gives neither
    // word a name, so a typo here still compiles and counts none.
    let sft_with = |reasoning: &str| report.sft.by_reasoning.get(reasoning).copied().unwrap_or(0);
    // A relative folder such as datasets/notes-v1 does not tell a person where it is.
    let folder = std::path::absolute(&report.folder).unwrap_or_else(|_| report.folder.clone());
    DatasetReport {
        name: report.name.clone(),
        folder,
        documents: report.documents.train + report.documents.heldout,
        sections_train: report.sections.train,
        sections_heldout: report.sections.heldout,
        rejected: report.rejected.values().sum(),
        synthetic: report.synthetic.total(),
        sft_think: sft_with("think"),
        sft_direct: sft_with("direct"),
        planned_calls: (!report.generation).then_some(report.planned.model_calls),
        usage: (&report.usage).into(),
    }
}
