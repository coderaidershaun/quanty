//! What a usage limit does: it stops the run, and the next run goes on from the cache.

use graph::testing::stored_concept_graph;
use rag_core::LlmError;
use rag_ingestion::{ConceptError, IngestError, Models, ingest_chapter};
use serde_json::{Value, json};

use super::script::{
    Named, PRICE_VARIABILITY, VOLATILITY, embedded_text, middle_score, prompt_file,
};
use super::{finding_volatility, items_of, positions_of};
use crate::support::{
    self, StandInEmbedder, StandInLlm, ThrowawayStores, assert_graph_holds_only, first_axis,
    points_in, vector_at,
};

fn chain_of(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(&format!(" / {cause}"));
        source = cause.source();
    }
    text
}

fn finding((name, definition): Named) -> Value {
    json!({ "concepts": [{ "name": name, "definition": definition }], "relations": [] })
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored concepts::"]
async fn a_usage_limit_stops_the_run_and_the_next_run_continues_from_the_cache() {
    let throwaway = ThrowawayStores::new("concepts-limit");
    let stores = throwaway.connect().await;
    let items = items_of(&support::intuition_chapter());
    let m = items.len();
    let positions = positions_of(&items);
    let limit_text = "You've hit your session limit · resets 11:40am";
    // The last item, so that every item before it is answered and kept before the stop is seen.
    let model = StandInLlm::replying(move |input, earlier_calls| {
        if positions[input] == m - 1 && earlier_calls == 0 {
            return Err(LlmError::UsageLimit {
                message: limit_text.to_owned(),
            });
        }
        Ok(finding_volatility())
    });
    let models = throwaway.models(model.clone());
    let chapter = support::intuition_chapter();

    let error = ingest_chapter(&chapter, &models, &stores)
        .await
        .unwrap_err();

    match &error {
        IngestError::Concepts(ConceptError::Stopped {
            read,
            items: asked_about,
            source: LlmError::UsageLimit { .. },
        }) => {
            assert_eq!((*read, *asked_about), (m - 1, m));
        }
        other => panic!("expected the run to stop at the usage limit, got {other:?}"),
    }
    let message = chain_of(&error);
    assert!(message.contains(limit_text), "{message}");
    assert!(message.contains("run the same command again"), "{message}");
    assert_eq!(model.calls(), m, "the limit is never asked again");
    assert_eq!(points_in(throwaway.config()).await.len(), m);
    assert_graph_holds_only(&stores.graph, &items).await;

    let second = ingest_chapter(&chapter, &models, &stores).await.unwrap();
    assert_eq!(model.calls(), m + 1);
    assert_eq!(second.concepts.llm_calls, 1);
    assert_eq!(second.concepts.cache_hits, m - 1);
    let stored = stored_concept_graph(&stores.graph).await;
    assert_eq!(stored.concepts.len(), 1);
    assert_eq!(stored.mentions.len(), m);

    // A model that is at its limit for every item, asked under another prompt version so that
    // nothing is answered from the cache. The first four questions are open before the first
    // answer is seen, and no item after them may be asked.
    let at_the_limit = StandInLlm::replying(move |_, _| {
        Err(LlmError::UsageLimit {
            message: limit_text.to_owned(),
        })
    });
    let models = throwaway.models(at_the_limit.clone());
    let models = Models {
        concepts: models.concepts.with_prompt_version("another version"),
        ..models
    };
    let error = ingest_chapter(&chapter, &models, &stores)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            IngestError::Concepts(ConceptError::Stopped { read: 0, .. })
        ),
        "{error:?}"
    );
    assert!(
        m > 4,
        "the chapter must have more items than are asked at once"
    );
    assert_eq!(at_the_limit.calls(), 4, "no item is asked after the stop");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored concepts::"]
async fn a_stop_while_the_concepts_are_linked_counts_the_item_that_was_skipped_before_it() {
    let throwaway = ThrowawayStores::new("concepts-link-stop");
    let stores = throwaway.connect().await;
    let items = items_of(&support::intuition_chapter());
    let positions = positions_of(&items);
    let same_concept_prompt = prompt_file("same-concept.md");
    // The first item makes a concept. The second is skipped, because no reply about it has the
    // right shape. The third names a concept that only the model can tell from the first, and
    // the model is at its limit by then, so the run stops while the third item is linked.
    let model = StandInLlm::replying_to_questions(move |question, _| {
        if question.system_prompt == same_concept_prompt {
            return Err(LlmError::UsageLimit {
                message: "You've hit your session limit".to_owned(),
            });
        }
        Ok(match positions[&question.input] {
            0 => finding(VOLATILITY),
            1 => json!({}),
            2 => finding(PRICE_VARIABILITY),
            _ => json!({ "concepts": [], "relations": [] }),
        })
    });
    let embedder = StandInEmbedder::default()
        .placing(&embedded_text(VOLATILITY), first_axis())
        .placing(
            &embedded_text(PRICE_VARIABILITY),
            vector_at(middle_score(), 2),
        );
    let models = throwaway.models_with_embedder(embedder, model);

    let error = ingest_chapter(&support::intuition_chapter(), &models, &stores)
        .await
        .unwrap_err();

    match &error {
        IngestError::Concepts(ConceptError::Stopped {
            read,
            items: asked_about,
            source: LlmError::UsageLimit { .. },
        }) => {
            assert_eq!(
                (*read, *asked_about),
                (2, items.len()),
                "the item that was linked and the item that was skipped are both done"
            );
        }
        other => panic!("expected the run to stop at the usage limit, got {other:?}"),
    }
}
