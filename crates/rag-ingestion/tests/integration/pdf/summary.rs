//! What a pdf run prints when it ends: the tokens of each model once, for the conversion and the
//! ingest together.

use ocr::testing::Scenario;
use rag_ingestion::PdfOutcome;

use super::{StandInPdf, finding_volatility};
use crate::support::{StandInLlm, ThrowawayStores};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored pdf::"]
async fn a_model_that_both_converted_and_read_the_concepts_has_one_line_in_the_summary() {
    let throwaway = ThrowawayStores::new("pdf-summary");
    let stores = throwaway.connect().await;
    let pdf = StandInPdf::new(&throwaway, Scenario::SampleChapter);
    // The stand-in that transcribes the pages has this name too.
    let model = StandInLlm::replying(|_, _| Ok(finding_volatility())).named("stub-transcriber");
    let models = throwaway.models(model);

    let outcome = pdf.ingest(&models, &stores).await.unwrap();

    let PdfOutcome::Ingested(summary) = &outcome else {
        panic!("expected the pdf to be ingested, got {outcome}");
    };
    let calls = summary.ingest.concepts.llm_calls;
    let printed = outcome.to_string();
    let lines: Vec<&str> = printed
        .lines()
        .filter(|line| line.starts_with("tokens of stub-transcriber: "))
        .collect();
    assert_eq!(
        lines,
        [format!(
            "tokens of stub-transcriber: {} in, {} out, 0 cache read, 0 cache write",
            800 + calls * 1000,
            80 + calls * 100
        )],
        "{printed}"
    );
}
