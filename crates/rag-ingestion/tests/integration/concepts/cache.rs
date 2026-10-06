//! What is kept between runs: a second run asks nothing, and another prompt version or another
//! model asks about every item again.

use graph::testing::{size, stored_concept_graph};
use rag_ingestion::{ConceptSummary, Models, ingest_chapter};

use super::{files_in, finding_volatility, items_of};
use crate::stand_in_llm::StandInLlm;
use crate::support::{self, ThrowawayStores};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored concepts::"]
async fn a_second_ingest_asks_nothing_and_another_prompt_version_asks_again() {
    let throwaway = ThrowawayStores::new("concepts-cache");
    let stores = throwaway.connect().await;
    let m = items_of(&support::intuition_chapter()).len();
    let model = StandInLlm::replying(|_, _| Ok(finding_volatility()));
    let models = throwaway.models(model.clone());
    let chapter = support::intuition_chapter();

    ingest_chapter(&chapter, &models, &stores).await.unwrap();
    assert_eq!(model.calls(), m);
    assert_eq!(files_in(&throwaway.config().concept_cache_folder), m);
    let size_after_first = size(&stores.graph).await;
    let graph_after_first = stored_concept_graph(&stores.graph).await;

    let second = ingest_chapter(&chapter, &models, &stores).await.unwrap();
    assert_eq!(model.calls(), m, "a second run asks nothing");
    assert_eq!(
        second.concepts,
        ConceptSummary {
            concepts_created: 0,
            concepts_linked: m,
            mentions_written: m,
            relations_written: 0,
            relations_dropped: 0,
            llm_calls: 0,
            cache_hits: m,
            skipped_items: Vec::new(),
        }
    );
    assert_eq!(size(&stores.graph).await, size_after_first);
    assert_eq!(
        stored_concept_graph(&stores.graph).await,
        graph_after_first,
        "the concept keeps its id"
    );

    let models = Models {
        concepts: models.concepts.with_prompt_version("another version"),
        ..models
    };
    let third = ingest_chapter(&chapter, &models, &stores).await.unwrap();
    assert_eq!(model.calls(), 2 * m, "another prompt version asks again");
    assert_eq!(third.concepts.llm_calls, m);
    assert_eq!(third.concepts.cache_hits, 0);
    assert_eq!(size(&stores.graph).await, size_after_first);
    assert_eq!(stored_concept_graph(&stores.graph).await, graph_after_first);

    // The first prompt version again, so only the name of the model differs from the first run.
    let other_model = StandInLlm::replying(|_, _| Ok(finding_volatility())).named("another model");
    let models = throwaway.models(other_model.clone());
    let fourth = ingest_chapter(&chapter, &models, &stores).await.unwrap();
    assert_eq!(other_model.calls(), m, "another model asks again");
    assert_eq!(fourth.concepts.cache_hits, 0);
}
