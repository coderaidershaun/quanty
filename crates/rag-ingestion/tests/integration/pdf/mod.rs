//! Runs a chapter pdf from the file to the stores, through `ingest_pdf` and through the built
//! `rag-ingest pdf` command. Nothing is billed: `ingest_pdf` converts with stand-ins for the paid
//! calls, the built command is stopped before its first paid call, the stores are throwaway ones
//! or ports where nothing listens, and the content folder is a temporary one.

mod command;
mod runs;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use graph::FalkorGraph;
use graph::testing::{
    GraphSize, StoredConceptGraph, StoredDocument, size, stored_concept_graph, stored_document,
};
use ocr::ChapterJob;
use ocr::convert::convert_chapter_with;
use ocr::testing::{Scenario, StubServices, sample_job};
use rag_core::{DocId, Llm, LlmError};
use rag_ingestion::testing::StandInEmbedder;
use rag_ingestion::{ChapterPdf, Models, PdfError, PdfOutcome, Stores, ingest_pdf};
use serde_json::{Value, json};

use crate::support::{StandInLlm, ThrowawayStores, concept_points_in, decisions_in, points_in};

/// The sample chapter pdf, which stand-ins for the paid calls of `ocr` convert.
struct StandInPdf {
    job: ChapterJob,
    stubs: StubServices,
    conversions_started: AtomicUsize,
}

impl StandInPdf {
    fn new(throwaway: &ThrowawayStores, scenario: Scenario) -> StandInPdf {
        StandInPdf {
            job: sample_job(&throwaway.config().content_folder),
            stubs: StubServices::new(scenario),
            conversions_started: AtomicUsize::new(0),
        }
    }

    async fn ingest<L: Llm>(
        &self,
        models: &Models<StandInEmbedder, L>,
        stores: &Stores<FalkorGraph>,
    ) -> Result<PdfOutcome, PdfError> {
        let pdf = ChapterPdf {
            job: &self.job,
            convert: async |job: &ChapterJob| {
                self.conversions_started.fetch_add(1, Ordering::SeqCst);
                convert_chapter_with(job, &self.stubs).await
            },
        };
        ingest_pdf(pdf, models, stores).await
    }

    fn conversions_started(&self) -> usize {
        self.conversions_started.load(Ordering::SeqCst)
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

/// A language model that is at its usage limit for every question.
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

/// What the stand-in prints as the page number of a page: its position, except that page 6 shows
/// 9 and page 7 shows 10.
fn printed_page_of(position: u64) -> String {
    match position {
        6 => "9".to_owned(),
        7 => "10".to_owned(),
        other => other.to_string(),
    }
}
