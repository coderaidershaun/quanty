//! One run over a chapter that has every kind of item: what the model is asked, and what its
//! answers become in the graph and in the summary.

use std::collections::BTreeSet;

use graph::RelationKind;
use graph::testing::{GraphSize, size, stored_concept_graph};
use rag_core::ConceptId;
use rag_ingestion::{ConceptSummary, ingest_chapter};
use serde_json::{Value, json};

use super::{input_of, items_of, positions_of};
use crate::support::{self, StandInLlm, ThrowawayStores};

/// Five ways to write one name: an en dash, a hyphen, a space, an em dash with a full stop, and
/// another hyphen character with spaces around and inside.
const SPELLINGS: [&str; 5] = [
    "Black–Scholes model",
    "black-scholes model",
    "Black Scholes Model",
    "BLACK—SCHOLES MODEL.",
    " Black‐Scholes  model ",
];

/// An ingest must be able to run as a spawned task. This compiles only while the future it is
/// given can be sent to another thread.
fn assert_send<T: Send>(value: T) -> T {
    value
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored concepts::"]
async fn ingest_asks_about_every_item_and_writes_concepts_mentions_and_relations() {
    let relation_types = [
        "DERIVED_FROM",
        "ASSUMES",
        "GENERALISES",
        "PART_OF",
        "USED_FOR",
    ];
    assert_eq!(RelationKind::ALL.map(RelationKind::as_str), relation_types);

    let throwaway = ThrowawayStores::new("concepts");
    let stores = throwaway.connect().await;
    let items = items_of(&support::sample_chapter());
    let n = items.len();
    let kinds_of_items: BTreeSet<&str> = items
        .iter()
        .map(|item| item.payload.kind.as_str())
        .collect();
    assert_eq!(
        kinds_of_items,
        BTreeSet::from(["chunk", "figure", "formula", "table"])
    );
    let positions = positions_of(&items);
    let model = StandInLlm::replying(move |input, _| {
        let i = positions[input];
        let spelling = SPELLINGS[i % 5];
        let mut relations = vec![json!({
            "from": format!("Itô's idea {i}"), "type": relation_types[i % 5], "to": spelling,
        })];
        if i >= 1 {
            relations.push(json!({
                "from": format!("Itô's idea {i}"),
                "type": "DERIVED_FROM",
                "to": format!("ITÔ'S IDEA {}", i - 1),
            }));
        } else {
            relations.push(json!({
                "from": "Itô's idea 0", "type": "PART_OF", "to": "a concept that nobody named",
            }));
        }
        Ok(json!({
            "concepts": [
                { "name": spelling, "definition": format!("definition {i} of the model") },
                {
                    "name": format!("Itô's idea {i}"),
                    "definition": format!("idea {i}, written \\( x_{i} \\)"),
                },
            ],
            "relations": relations,
        }))
    });
    let models = throwaway.models(model.clone());

    let summary = assert_send(ingest_chapter(&support::sample_chapter(), &models, &stores))
        .await
        .unwrap();

    assert_eq!(model.calls(), n, "one question for each item");
    let questions = model.questions();
    let asked: BTreeSet<String> = questions
        .iter()
        .map(|question| question.input.clone())
        .collect();
    assert_eq!(asked, items.iter().map(input_of).collect::<BTreeSet<_>>());
    let prompt_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/ingest/concepts/prompts/extract.md");
    let prompt = std::fs::read_to_string(prompt_file).unwrap();
    for question in &questions {
        assert_eq!(
            question.system_prompt, prompt,
            "every question carries the prompt file"
        );
        let schema: Value = serde_json::from_str(&question.schema).unwrap();
        let types = schema
            .pointer("/properties/relations/items/properties/type/enum")
            .expect("the schema lists the relation types");
        assert_eq!(
            types,
            &json!(relation_types),
            "the schema allows the five types and no other"
        );
    }
    assert_eq!(model.most_calls_at_once(), 4);

    let stored = stored_concept_graph(&stores.graph).await;
    assert_eq!(stored.concepts.len(), n + 1);
    let ids: BTreeSet<&str> = stored
        .concepts
        .iter()
        .map(|concept| concept.id.as_str())
        .collect();
    assert_eq!(ids.len(), n + 1, "every concept has its own id");
    assert!(ids.iter().all(|id| id.parse::<ConceptId>().is_ok()));
    let concept = |normalised_name: &str| {
        stored
            .concepts
            .iter()
            .find(|concept| concept.normalised_name == normalised_name)
            .unwrap_or_else(|| panic!("no concept named {normalised_name}"))
    };
    let black_scholes = concept("black scholes model");
    assert_eq!(black_scholes.name, "Black–Scholes model");
    assert_eq!(black_scholes.definition, "definition 0 of the model");
    assert!(black_scholes.aliases.is_empty());
    for i in 0..n {
        let idea = concept(&format!("itô s idea {i}"));
        assert_eq!(idea.name, format!("Itô's idea {i}"));
        assert_eq!(idea.definition, format!("idea {i}, written \\( x_{i} \\)"));
    }

    let mut mentions: Vec<(String, String, String)> = stored
        .mentions
        .iter()
        .map(|mention| {
            (
                mention.item.clone(),
                mention.concept.clone(),
                mention.wording.clone(),
            )
        })
        .collect();
    let mut expected_mentions = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let id = item.id.to_string();
        expected_mentions.push((
            id.clone(),
            "black scholes model".to_owned(),
            SPELLINGS[i % 5].trim().to_owned(),
        ));
        expected_mentions.push((id, format!("itô s idea {i}"), format!("Itô's idea {i}")));
    }
    mentions.sort();
    expected_mentions.sort();
    assert_eq!(mentions, expected_mentions);

    let mut relations: Vec<(String, String, String, String)> = stored
        .relations
        .iter()
        .map(|relation| {
            (
                relation.from.clone(),
                relation.kind.clone(),
                relation.to.clone(),
                relation.item.clone(),
            )
        })
        .collect();
    let mut expected_relations = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let id = item.id.to_string();
        expected_relations.push((
            format!("itô s idea {i}"),
            relation_types[i % 5].to_owned(),
            "black scholes model".to_owned(),
            id.clone(),
        ));
        if i >= 1 {
            expected_relations.push((
                format!("itô s idea {i}"),
                "DERIVED_FROM".to_owned(),
                format!("itô s idea {}", i - 1),
                id,
            ));
        }
    }
    relations.sort();
    expected_relations.sort();
    assert_eq!(relations, expected_relations);

    let nodes = 2 * (n as u64 + 1);
    let edges = n as u64 + (n as u64 - 1) + 2 * n as u64 + (2 * n as u64 - 1);
    assert_eq!(size(&stores.graph).await, GraphSize { nodes, edges });

    assert_eq!(
        summary.concepts,
        ConceptSummary {
            concepts_created: n + 1,
            concepts_linked: n - 1,
            mentions_written: 2 * n,
            relations_written: 2 * n - 1,
            relations_dropped: 1,
            llm_calls: n,
            cache_hits: 0,
            skipped_items: Vec::new(),
        }
    );
    let printed = summary.to_string();
    let concept_lines: Vec<&str> = printed.lines().skip(4).collect();
    assert_eq!(
        concept_lines,
        [
            format!(
                "concepts: {} created, {} linked to an existing concept",
                n + 1,
                n - 1
            ),
            format!("mentions written: {}", 2 * n),
            format!("relations written: {}, dropped: 1", 2 * n - 1),
            format!("claude calls made: {n}, cache hits: 0"),
            "items skipped: 0".to_owned(),
        ]
    );
}
