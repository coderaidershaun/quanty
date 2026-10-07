//! The chapter, the model and the embedder that the resolution tests follow: each item of the
//! intuition chapter names the concepts of its place in a script, a model answers a question about
//! two concepts as the test says, and an embedder puts the names at known distances from each
//! other.

use std::path::Path;

use rag_core::DocumentInput;
use rag_ingestion::{ASK_SCORE, Item, LINK_SCORE, Models, Stores};
use serde_json::{Value, json};

use super::{items_of, positions_of};
use crate::support::{self, StandInEmbedder, StandInLlm, ThrowawayStores, first_axis, vector_at};

/// A concept name and the definition that goes with it.
pub(super) type Named = (&'static str, &'static str);

pub(super) const VOLATILITY: Named = ("volatility", "how much a price moves over time");
pub(super) const WRITTEN_AGAIN: Named = ("Volatility.", "the size of the moves of a price");
pub(super) const VOL: Named = ("vol", "short for volatility");
pub(super) const PRICE_VARIABILITY: Named = ("price variability", "how widely a price varies");
pub(super) const REALISED_VARIANCE: Named = ("realised variance", "the squared moves a price made");
pub(super) const INTEREST_RATE: Named = ("interest rate", "the price of borrowing money");

/// The concepts that each item of the chapter names, in reading order and in the order of the
/// reply. The last item finds nothing. One reply never holds the same normalised name twice.
pub(super) const SCRIPT: [&[Named]; 8] = [
    &[VOLATILITY],
    &[WRITTEN_AGAIN],
    &[VOLATILITY, VOL],
    &[VOL],
    &[PRICE_VARIABILITY],
    &[REALISED_VARIANCE],
    &[INTEREST_RATE],
    &[],
];

pub(super) fn prompt_file(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/ingest/concepts/prompts")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

/// Answers each item with the concepts of its place in the script, and a question about two
/// concepts with "same" for price variability and "different" for anything else.
fn scripted_llm(items: &[Item]) -> StandInLlm {
    let positions = positions_of(items);
    let same_concept_prompt = prompt_file("same-concept.md");
    StandInLlm::replying_to_questions(move |question, _| {
        if question.system_prompt == same_concept_prompt {
            return Ok(json!({ "same": question.input.contains("name: price variability") }));
        }
        let concepts: Vec<Value> = SCRIPT[positions[&question.input]]
            .iter()
            .map(|(name, definition)| json!({ "name": name, "definition": definition }))
            .collect();
        Ok(json!({ "concepts": concepts, "relations": [] }))
    })
}

/// Between `LINK_SCORE` and the best score.
pub(super) fn high_score() -> f32 {
    (LINK_SCORE + 1.0) / 2.0
}

/// Between the two thresholds.
pub(super) fn middle_score() -> f32 {
    (ASK_SCORE + LINK_SCORE) / 2.0
}

pub(super) fn embedded_text((name, definition): Named) -> String {
    format!("{name}: {definition}")
}

/// Puts each name that is not an exact match where its rule needs it, measured from volatility:
/// vol over the high threshold, the other two between the thresholds. Interest rate is left to
/// the default, which is far from everything.
fn scripted_embedder() -> StandInEmbedder {
    StandInEmbedder::default()
        .placing(&embedded_text(VOLATILITY), first_axis())
        .placing(&embedded_text(VOL), vector_at(high_score(), 1))
        .placing(
            &embedded_text(PRICE_VARIABILITY),
            vector_at(middle_score(), 2),
        )
        .placing(
            &embedded_text(REALISED_VARIANCE),
            vector_at(middle_score(), 3),
        )
}

/// The inputs that the embedder was given for concepts. An item always has a title, and a concept
/// never has one.
pub(super) fn concept_inputs(embedder: &StandInEmbedder) -> Vec<DocumentInput> {
    let mut inputs = embedder.received();
    inputs.retain(|input| input.title.is_empty());
    inputs
}

/// Throwaway stores and the items of the intuition chapter, for models that follow the script.
pub(super) struct Scripted {
    pub throwaway: ThrowawayStores,
    pub stores: Stores<graph::FalkorGraph>,
    pub items: Vec<Item>,
}

pub(super) async fn scripted(test_name: &str) -> Scripted {
    let throwaway = ThrowawayStores::new(test_name);
    let stores = throwaway.connect().await;
    let items = items_of(&support::intuition_chapter());
    assert_eq!(
        items.len(),
        SCRIPT.len(),
        "one entry of the script for each item"
    );
    Scripted {
        throwaway,
        stores,
        items,
    }
}

impl Scripted {
    /// The models of one ingest, and the model on its own to read what it was asked. Every call
    /// makes a model and an embedder of its own, and they share the cache folder and the decision
    /// log of the throwaway stores.
    pub(super) fn models(&self) -> (Models<StandInEmbedder, StandInLlm>, StandInLlm) {
        let model = scripted_llm(&self.items);
        let models = self
            .throwaway
            .models_with_embedder(scripted_embedder(), model.clone());
        (models, model)
    }
}
