//! Runs a pdf from the file to the stores through `ingest_pdf`. Nothing is billed: it converts
//! with stand-ins for the paid calls, the stores are throwaway ones, and the content folder is a
//! temporary one.

mod runs;

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use graph::FalkorGraph;
use graph::testing::{
    GraphSize, StoredConceptGraph, StoredDocument, size, stored_concept_graph, stored_document,
};
use ocr::convert::convert_chapter_with_progress;
use ocr::testing::{Scenario, StubServices, sample_job};
use ocr::{ChapterJob, PageProgress};
use rag_core::{DocId, Llm, LlmError, MediaLabels, UsageTally};
use rag_ingestion::testing::StandInEmbedder;
use rag_ingestion::{ChapterPdf, IngestStep, Models, PdfError, PdfOutcome, Stores, ingest_pdf};
use serde_json::{Value, json};

use crate::support::{StandInLlm, ThrowawayStores, concept_points_in, decisions_in, points_in};

/// The sample chapter pdf, which stand-ins for the paid calls of `ocr` convert.
struct StandInPdf {
    job: ChapterJob,
    new_media: MediaLabels,
    stubs: StubServices,
    conversions_started: AtomicUsize,
    /// Each step with what the run had used when it was told.
    steps: Mutex<Vec<(IngestStep, UsageTally)>>,
}

impl StandInPdf {
    /// The sample book chapter, whose media is made with no labels.
    fn new(throwaway: &ThrowawayStores, scenario: Scenario) -> StandInPdf {
        StandInPdf::of(
            sample_job(&throwaway.config().content_folder),
            MediaLabels::default(),
            scenario,
        )
    }

    fn of(job: ChapterJob, new_media: MediaLabels, scenario: Scenario) -> StandInPdf {
        StandInPdf {
            job,
            new_media,
            stubs: StubServices::new(scenario),
            conversions_started: AtomicUsize::new(0),
            steps: Mutex::new(Vec::new()),
        }
    }

    async fn ingest<L: Llm>(
        &self,
        models: &Models<StandInEmbedder, L>,
        stores: &Stores<FalkorGraph>,
    ) -> Result<PdfOutcome, PdfError> {
        let pdf = ChapterPdf {
            job: &self.job,
            new_media: &self.new_media,
            convert:
                async |job: &ChapterJob, on_page: &mut (dyn FnMut(PageProgress) + Send + '_)| {
                    self.conversions_started.fetch_add(1, Ordering::SeqCst);
                    convert_chapter_with_progress(job, &self.stubs, on_page).await
                },
            on_step: |step, spent: &UsageTally| {
                self.steps.lock().unwrap().push((step, spent.clone()));
            },
        };
        ingest_pdf(pdf, models, stores).await
    }

    fn conversions_started(&self) -> usize {
        self.conversions_started.load(Ordering::SeqCst)
    }

    /// Every step that the runs of this pdf told, in the order they came.
    fn steps(&self) -> Vec<IngestStep> {
        let steps = self.steps.lock().unwrap();
        steps.iter().map(|(step, _)| step.clone()).collect()
    }

    /// What the runs had used at each step, in the order the steps came.
    fn spent(&self) -> Vec<UsageTally> {
        let steps = self.steps.lock().unwrap();
        steps.iter().map(|(_, spent)| spent.clone()).collect()
    }
}

/// Everything that a run that does nothing must leave as it is.
#[derive(Debug, PartialEq)]
struct Held {
    points: BTreeMap<String, Value>,
    concept_points: BTreeMap<String, Value>,
    graph_size: GraphSize,
    concept_graph: StoredConceptGraph,
    document: Option<StoredDocument>,
    decisions: Vec<Value>,
    cached_answers: usize,
}

async fn held_by(
    throwaway: &ThrowawayStores,
    stores: &Stores<FalkorGraph>,
    document: DocId,
) -> Held {
    let config = throwaway.config();
    Held {
        points: points_in(config).await.into_iter().collect(),
        concept_points: concept_points_in(config).await.into_iter().collect(),
        graph_size: size(&stores.graph).await,
        concept_graph: stored_concept_graph(&stores.graph).await,
        document: stored_document(&stores.graph, document).await,
        decisions: decisions_in(config),
        cached_answers: std::fs::read_dir(&config.concept_cache_folder)
            .unwrap()
            .count(),
    }
}

/// Every answer names the same concept, so the first item makes it and each other item links to
/// it.
fn finding_volatility() -> Value {
    json!({
        "concepts": [{ "name": "volatility", "definition": "how much a price moves over time" }],
        "relations": [],
    })
}

fn at_the_usage_limit() -> StandInLlm {
    StandInLlm::replying(|_, _| {
        Err(LlmError::UsageLimit {
            message: "You've hit your session limit".to_owned(),
        })
    })
}

fn chain_of(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(&format!(" / {cause}"));
        source = cause.source();
    }
    text
}

fn printed_page_of(position: u64) -> String {
    match position {
        6 => "9".to_owned(),
        7 => "10".to_owned(),
        other => other.to_string(),
    }
}
