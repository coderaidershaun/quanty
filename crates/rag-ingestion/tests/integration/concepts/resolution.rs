//! Resolution over one chapter with a model and an embedder that follow a script: each rule links
//! or makes a concept and logs why, and a second run asks nothing and writes nothing.

use std::collections::BTreeSet;

use graph::testing::{GraphSize, StoredConceptGraph, size, stored_concept_graph};
use rag_core::ConceptId;
use rag_ingestion::{ASK_SCORE, ConceptSummary, Item, ingest_chapter};
use serde_json::{Value, json};

use super::script::{
    INTEREST_RATE, Named, PRICE_VARIABILITY, REALISED_VARIANCE, SCRIPT, Scripted, VOL, VOLATILITY,
    concept_inputs, embedded_text, high_score, middle_score, prompt_file, scripted,
};
use crate::support::{self, concept_points_in, decision_for_mention, decisions_in, mentions_of};

fn id_of(stored: &StoredConceptGraph, normalised_name: &str) -> String {
    stored
        .concepts
        .iter()
        .find(|concept| concept.normalised_name == normalised_name)
        .unwrap_or_else(|| panic!("no concept named {normalised_name}"))
        .id
        .clone()
}

fn concept_ref(stored: &StoredConceptGraph, normalised_name: &str, name: &str) -> Value {
    json!({ "id": id_of(stored, normalised_name), "name": name })
}

fn keys_of(line: &Value) -> BTreeSet<&str> {
    line.as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

fn assert_score(line: &Value, expected: f32) {
    let score = line["score"].as_f64().expect("the line has a score") as f32;
    assert!(
        (score - expected).abs() < 1e-3,
        "{score} is not {expected}: {line}"
    );
}

/// The fields every line has, and the key set that the rule gives it.
fn assert_decision(line: &Value, item: &Item, name: &str, rule: &str, keys: &[&str]) {
    assert_eq!(line["item"], item.id.to_string(), "{line}");
    assert_eq!(line["name"], name, "{line}");
    assert_eq!(line["rule"], rule, "{line}");
    let mut expected: BTreeSet<&str> = BTreeSet::from(["item", "name", "rule"]);
    expected.extend(keys);
    assert_eq!(
        keys_of(line),
        expected,
        "a field that does not belong, or is missing: {line}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored concepts::"]
async fn each_rule_of_resolution_links_or_creates_and_logs_its_decision() {
    let run = scripted("resolution").await;
    let (models, model) = run.models();
    let (throwaway, stores, items) = (&run.throwaway, &run.stores, &run.items);

    let summary = ingest_chapter(&support::intuition_chapter(), &models, stores)
        .await
        .unwrap();
    let stored = stored_concept_graph(&stores.graph).await;

    let names: Vec<&str> = stored
        .concepts
        .iter()
        .map(|concept| concept.normalised_name.as_str())
        .collect();
    assert_eq!(names, ["interest rate", "realised variance", "volatility"]);
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
    // Item 3 names volatility twice and has one mention, with the first wording.
    let mut expected_mentions: Vec<(String, String, String)> = [
        (0, "volatility", "volatility"),
        (1, "volatility", "Volatility."),
        (2, "volatility", "volatility"),
        (3, "volatility", "vol"),
        (4, "volatility", "price variability"),
        (5, "realised variance", "realised variance"),
        (6, "interest rate", "interest rate"),
    ]
    .map(|(item, concept, wording)| {
        (
            items[item].id.to_string(),
            concept.to_owned(),
            wording.to_owned(),
        )
    })
    .into();
    mentions.sort();
    expected_mentions.sort();
    assert_eq!(mentions, expected_mentions);
    assert_eq!(
        summary.concepts,
        ConceptSummary {
            concepts_created: 3,
            concepts_linked: 5,
            mentions_written: 7,
            relations_written: 0,
            relations_dropped: 0,
            llm_calls: 10,
            cache_hits: 0,
            skipped_items: Vec::new(),
        }
    );
    let same_concept_prompt = prompt_file("same-concept.md");
    let comparisons: Vec<_> = model
        .questions()
        .into_iter()
        .filter(|question| question.system_prompt == same_concept_prompt)
        .collect();
    assert_eq!(model.calls(), SCRIPT.len() + comparisons.len());
    let comparison_input = |new: Named, stored: Named| {
        format!(
            "First concept\nname: {}\ndefinition: {}\n\nSecond concept\nname: {}\ndefinition: {}",
            new.0, new.1, stored.0, stored.1
        )
    };
    assert_eq!(
        comparisons
            .iter()
            .map(|question| question.input.clone())
            .collect::<Vec<_>>(),
        [
            comparison_input(PRICE_VARIABILITY, VOLATILITY),
            comparison_input(REALISED_VARIANCE, VOLATILITY),
        ],
        "both names and both definitions go to the model, the stored definition as it was first written"
    );
    for question in &comparisons {
        assert_eq!(question.schema, prompt_file("same-concept.schema.json"));
    }

    let aliases = |normalised_name: &str| {
        stored
            .concepts
            .iter()
            .find(|concept| concept.normalised_name == normalised_name)
            .unwrap()
            .aliases
            .clone()
    };
    assert_eq!(aliases("volatility"), ["vol", "price variability"]);
    assert!(aliases("realised variance").is_empty());
    assert!(aliases("interest rate").is_empty());

    let points = concept_points_in(throwaway.config()).await;
    assert_eq!(points.len(), 3);
    let node_ids: BTreeSet<&str> = stored
        .concepts
        .iter()
        .map(|concept| concept.id.as_str())
        .collect();
    assert_eq!(
        points
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<BTreeSet<_>>(),
        node_ids
    );
    assert!(node_ids.iter().all(|id| id.parse::<ConceptId>().is_ok()));
    for (id, payload) in &points {
        let node = stored
            .concepts
            .iter()
            .find(|concept| &concept.id == id)
            .unwrap();
        assert_eq!(
            payload,
            &json!({ "name": node.name, "aliases": node.aliases }),
            "the payload holds the name and the aliases and nothing else"
        );
    }
    let embedded: Vec<(String, String, bool)> = concept_inputs(&models.embedder)
        .into_iter()
        .map(|input| (input.title, input.text, input.image.is_some()))
        .collect();
    assert_eq!(
        embedded,
        [
            VOLATILITY,
            VOL,
            PRICE_VARIABILITY,
            REALISED_VARIANCE,
            INTEREST_RATE
        ]
        .map(|named| (String::new(), embedded_text(named), false)),
        "an empty title, no picture, and nothing for the names that matched exactly"
    );

    let log = decisions_in(throwaway.config());
    assert_eq!(log.len(), 8);
    let volatility = concept_ref(&stored, "volatility", "volatility");
    let line = &log[0];
    assert_decision(
        line,
        &items[0],
        "volatility",
        "nothing-stored",
        &["created"],
    );
    assert_eq!(line["created"], id_of(&stored, "volatility"));
    for (line, item, name) in [
        (&log[1], &items[1], "Volatility."),
        (&log[2], &items[2], "volatility"),
        (&log[4], &items[3], "vol"),
    ] {
        assert_decision(line, item, name, "exact-name", &["matched"]);
        assert_eq!(line["matched"], volatility);
    }
    let line = &log[3];
    assert_decision(line, &items[2], "vol", "high-score", &["matched", "score"]);
    assert_eq!(line["matched"], volatility);
    assert_score(line, high_score());
    let line = &log[5];
    assert_decision(
        line,
        &items[4],
        "price variability",
        "llm-same",
        &["matched", "score"],
    );
    assert_eq!(line["matched"], volatility);
    assert_score(line, middle_score());
    let line = &log[6];
    let created = ["nearest", "score", "created"];
    assert_decision(
        line,
        &items[5],
        "realised variance",
        "llm-different",
        &created,
    );
    assert_eq!(line["nearest"], volatility);
    assert_score(line, middle_score());
    assert_eq!(line["created"], id_of(&stored, "realised variance"));
    let line = &log[7];
    assert_decision(line, &items[6], "interest rate", "low-score", &created);
    assert_eq!(line["created"], id_of(&stored, "interest rate"));
    assert!(line["nearest"]["id"].is_string() && line["nearest"]["name"].is_string());
    let score = line["score"].as_f64().unwrap() as f32;
    assert!(score < ASK_SCORE, "{line}");

    for item in items {
        for mention in mentions_of(&stored, &item.id.to_string()) {
            let concept = id_of(&stored, &mention.concept);
            assert!(
                decision_for_mention(&log, &mention.item, &concept).is_some(),
                "no line of the log explains {mention:?}"
            );
        }
    }
}

/// What the stores hold, so that a second run can be compared with the first.
struct Held {
    size: GraphSize,
    graph: StoredConceptGraph,
    points: Vec<(String, Value)>,
}

async fn held_by(scripted: &Scripted) -> Held {
    let mut points = concept_points_in(scripted.throwaway.config()).await;
    points.sort_by(|(first, _), (second, _)| first.cmp(second));
    Held {
        size: size(&scripted.stores.graph).await,
        graph: stored_concept_graph(&scripted.stores.graph).await,
        points,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored concepts::"]
async fn a_second_ingest_asks_nothing_and_adds_no_node_edge_point_or_alias() {
    let run = scripted("resolution-again").await;
    let (models, model) = run.models();
    let chapter = support::intuition_chapter();

    ingest_chapter(&chapter, &models, &run.stores)
        .await
        .unwrap();
    let calls_after_first = model.calls();
    let held_after_first = held_by(&run).await;
    let concept_inputs_after_first = concept_inputs(&models.embedder).len();
    let lines_after_first = decisions_in(run.throwaway.config()).len();

    let second = ingest_chapter(&chapter, &models, &run.stores)
        .await
        .unwrap();

    assert_eq!(
        model.calls(),
        calls_after_first,
        "a second run asks the model nothing: no extraction question and no comparison"
    );
    assert_eq!(
        second.concepts,
        ConceptSummary {
            concepts_created: 0,
            concepts_linked: 8,
            mentions_written: 7,
            relations_written: 0,
            relations_dropped: 0,
            llm_calls: 0,
            cache_hits: SCRIPT.len(),
            skipped_items: Vec::new(),
        }
    );
    let held_after_second = held_by(&run).await;
    assert_eq!(held_after_second.size, held_after_first.size);
    assert_eq!(
        held_after_second.graph, held_after_first.graph,
        "the same concepts, aliases, mentions and relations"
    );
    assert_eq!(held_after_second.points, held_after_first.points);
    assert_eq!(
        concept_inputs(&models.embedder).len(),
        concept_inputs_after_first,
        "no concept is embedded when every name matches exactly"
    );
    let log = decisions_in(run.throwaway.config());
    assert_eq!(log.len(), lines_after_first + 8);
    assert!(
        log[lines_after_first..]
            .iter()
            .all(|line| line["rule"] == "exact-name"),
        "every name of the second run matches exactly"
    );
}
