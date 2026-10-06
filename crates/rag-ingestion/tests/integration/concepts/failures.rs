//! What a failure of the model does: an item that fails twice is skipped and asked again on the
//! next run, and a usage limit stops the run, which the next run goes on from.

use std::collections::BTreeSet;

use graph::testing::{StoredConceptGraph, stored_concept_graph};
use rag_core::LlmError;
use rag_ingestion::{ConceptError, IngestError, Models, ingest_chapter};
use serde_json::json;

use super::{files_in, finding_volatility, items_of, positions_of};
use crate::stand_in_llm::StandInLlm;
use crate::support::{self, ThrowawayStores, assert_graph_holds_only, points_in};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored concepts::"]
async fn an_item_that_fails_twice_is_skipped_named_and_asked_again_on_the_next_run() {
    let throwaway = ThrowawayStores::new("concepts-skip");
    let stores = throwaway.connect().await;
    let items = items_of(&support::intuition_chapter());
    let m = items.len();
    let positions = positions_of(&items);
    let model =
        StandInLlm::replying(
            move |input, earlier_calls| match (positions[input], earlier_calls) {
                (0, 0) => Err(LlmError::Failed {
                    exit_code: Some(1),
                    reason: "Output blocked by content filtering policy".to_owned(),
                }),
                (1, 0) => Err(LlmError::TimedOut { seconds: 120 }),
                (1, 1) => Ok(json!({
                    "concepts": [{ "name": "volatility", "definition": "how much a price moves" }],
                    "relations": [{ "from": "volatility", "type": "CAUSES", "to": "volatility" }],
                })),
                _ => Ok(finding_volatility()),
            },
        );
    let models = throwaway.models(model.clone());
    let chapter = support::intuition_chapter();
    let second_item = &items[1];

    let first = ingest_chapter(&chapter, &models, &stores).await.unwrap();

    assert_eq!(model.calls(), m + 2);
    assert_eq!(first.concepts.llm_calls, m + 2);
    assert_eq!(first.concepts.cache_hits, 0);
    assert_eq!(
        files_in(&throwaway.config().concept_cache_folder),
        m - 1,
        "the reply that was refused is not kept"
    );
    let skipped = &first.concepts.skipped_items;
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert_eq!(skipped[0].id, second_item.id);
    assert_eq!(skipped[0].kind, second_item.payload.kind);
    assert_eq!(skipped[0].page, second_item.payload.page);
    assert!(
        skipped[0].reason.contains("CAUSES"),
        "{}",
        skipped[0].reason
    );
    let printed = first.to_string();
    let lines: Vec<&str> = printed.lines().collect();
    let skipped_line = lines
        .iter()
        .position(|line| *line == "items skipped: 1")
        .expect("the summary counts the skipped item");
    let named = lines[skipped_line + 1];
    for part in [
        second_item.payload.kind.as_str(),
        &format!("page {}", second_item.payload.page),
        &second_item.id.to_string(),
    ] {
        assert!(named.contains(part), "{part} is missing from: {named}");
    }
    let mentioning = |stored: &StoredConceptGraph| -> BTreeSet<String> {
        stored
            .mentions
            .iter()
            .map(|mention| mention.item.clone())
            .collect()
    };
    let others: BTreeSet<String> = items
        .iter()
        .filter(|item| item.id != second_item.id)
        .map(|item| item.id.to_string())
        .collect();
    assert_eq!(
        mentioning(&stored_concept_graph(&stores.graph).await),
        others
    );

    let second = ingest_chapter(&chapter, &models, &stores).await.unwrap();
    assert_eq!(model.calls(), m + 3, "only the skipped item is asked again");
    assert_eq!(second.concepts.llm_calls, 1);
    assert_eq!(second.concepts.cache_hits, m - 1);
    assert!(second.concepts.skipped_items.is_empty());
    let everyone: BTreeSet<String> = items.iter().map(|item| item.id.to_string()).collect();
    assert_eq!(
        mentioning(&stored_concept_graph(&stores.graph).await),
        everyone
    );
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
