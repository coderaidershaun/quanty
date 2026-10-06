//! Runs the real `rag-ingest` command twice on the authored chapter about the Black–Scholes
//! model, with the real Gemini API, the real `claude` program, a throwaway collection of the
//! local Qdrant, a throwaway graph of the local FalkorDB and a temporary cache folder. Only that
//! shows that the binary wires the real model, the prompt, the schema and the cache together, and
//! that the concepts it finds make a sensible graph.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Output;

use graph::FalkorGraph;
use graph::RelationKind;
use graph::testing::{StoredConceptGraph, size, stored_concept_graph};
use ocr::read_chapter;
use rag_ingestion::chapter_items;

use crate::support::{self, ThrowawayStores};

const RUN_COMMAND: &str = "set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-ingestion --test integration -- --ignored concepts_live:: --nocapture";

fn require_prod_api() {
    assert_eq!(
        std::env::var("REX_PROD_API").as_deref(),
        Ok("true"),
        "this test calls the real Gemini API and the real claude program; run it with: {RUN_COMMAND}"
    );
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The concept graph in words that a person can judge: each concept with its definition and the
/// number of items that mention it, then each relation.
fn describe(stored: &StoredConceptGraph) -> String {
    let names: BTreeMap<&str, &str> = stored
        .concepts
        .iter()
        .map(|concept| (concept.normalised_name.as_str(), concept.name.as_str()))
        .collect();
    let mut text = format!("{} concepts:\n", stored.concepts.len());
    for concept in &stored.concepts {
        let mentioned_by = stored
            .mentions
            .iter()
            .filter(|mention| mention.concept == concept.normalised_name)
            .count();
        text.push_str(&format!(
            "  {} — {} (mentioned by {mentioned_by} items)\n",
            concept.name, concept.definition
        ));
    }
    text.push_str(&format!("{} relations:\n", stored.relations.len()));
    for relation in &stored.relations {
        text.push_str(&format!(
            "  {} —{}→ {}\n",
            names[relation.from.as_str()],
            relation.kind,
            names[relation.to.as_str()]
        ));
    }
    text
}

/// The number after `claude calls made: ` in the summary, and the number after `cache hits: `.
fn calls_and_cache_hits(stdout: &str) -> (usize, usize) {
    let line = stdout
        .lines()
        .find_map(|line| line.strip_prefix("claude calls made: "))
        .unwrap_or_else(|| panic!("the summary has no line about claude calls: {stdout}"));
    let (calls, hits) = line
        .split_once(", cache hits: ")
        .unwrap_or_else(|| panic!("a line about claude calls that is not understood: {line}"));
    (calls.parse().unwrap(), hits.parse().unwrap())
}

fn cost_lines(stderr: &str) -> usize {
    stderr
        .lines()
        .filter(|line| line.contains("cost_usd="))
        .count()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "calls the real Gemini API and the real claude program, one call for each item of the chapter (about 11), and spends API credit and subscription usage; run with: set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-ingestion --test integration -- --ignored concepts_live:: --nocapture"]
async fn rag_ingest_builds_a_concept_graph_live_and_a_second_run_asks_claude_nothing() {
    require_prod_api();
    let throwaway = ThrowawayStores::new("concepts-live");
    let config = throwaway.config();
    let items = chapter_items(&read_chapter(&support::in_depth_chapter()).unwrap());
    let item_count = items.len();
    let run = || throwaway.rag_ingest_asking_claude([support::in_depth_chapter()]);

    let first = run();
    let graph = FalkorGraph::connect(config).await.unwrap();
    let after_first = stored_concept_graph(&graph).await;
    // Printed before anything is checked, so that a failed check does not throw away what the
    // paid run made.
    println!("--- first run: stdout\n{}", stdout_of(&first));
    println!("--- first run: stderr\n{}", stderr_of(&first));
    println!(
        "--- the concept graph after the first run\n{}",
        describe(&after_first)
    );

    assert!(
        first.status.success(),
        "first run failed: {}",
        stderr_of(&first)
    );
    let stdout = stdout_of(&first);
    assert!(stdout.contains("items skipped: 0"), "{stdout}");
    let (calls, cache_hits) = calls_and_cache_hits(&stdout);
    assert!(calls >= item_count, "{calls} calls for {item_count} items");
    assert_eq!(cache_hits, 0);
    let costs = cost_lines(&stderr_of(&first));
    assert!(
        costs >= item_count,
        "{costs} cost lines for {item_count} items"
    );
    assert!(
        after_first
            .concepts
            .iter()
            .any(|concept| concept.normalised_name.contains("black scholes")),
        "no Black–Scholes concept"
    );
    assert!(!after_first.mentions.is_empty());
    let item_ids: BTreeSet<String> = items.iter().map(|item| item.id.to_string()).collect();
    assert!(
        after_first
            .mentions
            .iter()
            .all(|mention| item_ids.contains(&mention.item)),
        "a mention of something that is not an item of the chapter"
    );
    let kinds = RelationKind::ALL.map(RelationKind::as_str);
    assert!(
        after_first
            .relations
            .iter()
            .all(|relation| kinds.contains(&relation.kind.as_str())),
        "a relation of a kind that is not one of the five"
    );
    let size_after_first = size(&graph).await;

    let second = run();
    assert!(
        second.status.success(),
        "second run failed: {}",
        stderr_of(&second)
    );
    let stdout = stdout_of(&second);
    assert!(
        stdout.contains(&format!("claude calls made: 0, cache hits: {item_count}")),
        "{stdout}"
    );
    assert_eq!(
        cost_lines(&stderr_of(&second)),
        0,
        "a second run asks claude nothing"
    );
    assert_eq!(size(&graph).await, size_after_first);
    assert_eq!(stored_concept_graph(&graph).await, after_first);
}
