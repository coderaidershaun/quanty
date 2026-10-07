//! Runs the real `rag-ingest` command on the two authored chapters and then on one chart that
//! stands alone, with the real Gemini API, the real `claude` program, throwaway stores, and a
//! temporary decision log and content folder. Only that shows that real vectors, the prompts and
//! the picture path together make one Black–Scholes model of two chapters, and link a chart to
//! the concepts that the chapters already hold.
//!
//! The run is paid for once. So it prints what each step made as soon as the step ends, and it
//! makes every check and names all that failed together at the end.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::process::Output;

use graph::FalkorGraph;
use graph::testing::{StoredConcept, StoredConceptGraph, stored_concept_graph};
use ocr::read_chapter;
use rag_core::{Config, Embedder, GeminiEmbedder, ItemHit, ItemKind, ItemStore};
use rag_ingestion::{Item, chapter_items};
use serde_json::Value;

use crate::concepts_live::{calls_and_cache_hits, stderr_of, stdout_of};
use crate::support::{
    self, ThrowawayStores, decision_for_mention, decisions_in, mentions_of, points_in,
};

const RUN_COMMAND: &str = "set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-ingestion --test integration -- --ignored resolution_live:: --nocapture";
const NOTE: &str = "A chart from a book on option trading.";
const QUESTION: &str =
    "How does implied volatility change across exercise prices and months to expiration?";
const CHART_TITLE: &str = "volatility-surface.png";

fn require_prod_api() {
    assert_eq!(
        std::env::var("REX_PROD_API").as_deref(),
        Ok("true"),
        "this test calls the real Gemini API and the real claude program; run it with: {RUN_COMMAND}"
    );
}

/// `rag-ingest` does not start `claude` while the key is set, so every command of the run would
/// stop at its first question.
fn require_no_api_key() {
    assert!(
        std::env::var_os("ANTHROPIC_API_KEY").is_none_or(|key| key.is_empty()),
        "ANTHROPIC_API_KEY is set, so rag-ingest would not start claude; unset it and run again"
    );
}

fn items_of(chapter_folder: &std::path::Path) -> Vec<Item> {
    chapter_items(&read_chapter(chapter_folder).unwrap())
}

fn print_output(label: &str, output: &Output) {
    println!("--- {label}: stdout\n{}", stdout_of(output));
    println!("--- {label}: stderr\n{}", stderr_of(output));
}

/// The concepts in words that a person can judge: each one with its definition, its aliases and
/// the number of items that mention it.
fn describe_concepts(stored: &StoredConceptGraph) -> String {
    let mut text = format!("{} concepts:\n", stored.concepts.len());
    for concept in &stored.concepts {
        let mentioned_by = stored
            .mentions
            .iter()
            .filter(|mention| mention.concept == concept.normalised_name)
            .count();
        text.push_str(&format!(
            "  {} — {} (also known as: {}; mentioned by {mentioned_by} items)\n",
            concept.name,
            concept.definition,
            concept.aliases.join(", ")
        ));
    }
    text
}

fn concept_named<'a>(
    stored: &'a StoredConceptGraph,
    normalised_name: &str,
) -> Option<&'a StoredConcept> {
    stored
        .concepts
        .iter()
        .find(|concept| concept.normalised_name == normalised_name)
}

/// The concepts whose normalised name starts with `black scholes` and ends with `model`. A name
/// that only holds those words, such as "assumptions of the Black–Scholes model", is a concept
/// about the model and not the model itself, so it is not counted.
fn black_scholes_models(stored: &StoredConceptGraph) -> Vec<&StoredConcept> {
    stored
        .concepts
        .iter()
        .filter(|concept| {
            concept.normalised_name.starts_with("black scholes")
                && concept.normalised_name.ends_with("model")
        })
        .collect()
}

/// Every line of the log, how many lines each rule has, and every `high-score` line once more on
/// its own, because those lines are the evidence for the two thresholds.
fn print_decisions(label: &str, log: &[Value]) {
    println!("--- the decision log {label}: {} lines", log.len());
    for line in log {
        println!("{line}");
    }
    let mut rules: BTreeMap<&str, usize> = BTreeMap::new();
    for line in log {
        *rules
            .entry(line["rule"].as_str().unwrap_or("?"))
            .or_default() += 1;
    }
    println!("--- lines for each rule {label}: {rules:?}");
    println!("--- every high-score line {label} (the evidence for the thresholds)");
    for line in log.iter().filter(|line| line["rule"] == "high-score") {
        println!("{line}");
    }
}

/// Stops the run when a chapter was not ingested, because the chart and every check need both
/// chapters. The decision log is printed first: it is the one thing the command made that its own
/// output does not show, and it goes away with the temporary folder.
fn stop_unless_ingested(config: &Config, label: &str, output: &Output) {
    if output.status.success() {
        return;
    }
    print_decisions("when the run stopped", &decisions_in(config));
    panic!("{label} was not ingested: {}", stderr_of(output));
}

/// What is wrong with the Black–Scholes model that the two chapters made. Nothing is wrong when
/// it is one concept, items of both chapters mention it, and the log explains each mention.
fn model_faults(
    stored: &StoredConceptGraph,
    log: &[Value],
    chapters: [(&str, &[Item]); 2],
) -> Vec<String> {
    let models = black_scholes_models(stored);
    let [model] = models.as_slice() else {
        let names: Vec<&str> = models.iter().map(|model| model.name.as_str()).collect();
        return vec![format!(
            "there is not exactly one Black–Scholes model: {names:?}"
        )];
    };
    let mentioning: BTreeSet<&str> = stored
        .mentions
        .iter()
        .filter(|mention| mention.concept == model.normalised_name)
        .map(|mention| mention.item.as_str())
        .collect();
    let mut faults = Vec::new();
    for (chapter, items) in chapters {
        let is_mentioned = items
            .iter()
            .any(|item| mentioning.contains(item.id.to_string().as_str()));
        if !is_mentioned {
            faults.push(format!(
                "no item of the {chapter} chapter mentions the Black–Scholes model"
            ));
        }
    }
    for item in &mentioning {
        if decision_for_mention(log, item, &model.id).is_none() {
            faults.push(format!(
                "no line of the decision log explains the mention of the model by {item}"
            ));
        }
    }
    faults
}

/// The five items nearest to the question, found the way `rag-query` finds them.
async fn nearest_to_question(
    config: &Config,
    embedder: &GeminiEmbedder,
) -> anyhow::Result<Vec<ItemHit>> {
    let vector = embedder.embed_query(QUESTION).await?;
    Ok(ItemStore::connect(config)?.search(vector, None, 5).await?)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "calls the real Gemini API and the real claude program: about 20 questions about items and about 55 about two concepts for the two chapters, and about 4 more for the chart plus Sonnet once or twice, and spends API credit and subscription usage; run with: set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-ingestion --test integration -- --ignored resolution_live:: --nocapture"]
async fn two_chapters_share_one_black_scholes_model_and_a_lone_chart_links_to_their_concepts_live()
{
    require_prod_api();
    require_no_api_key();
    let throwaway = ThrowawayStores::new("resolution-live");
    let config = throwaway.config();
    let intuition = items_of(&support::intuition_chapter());
    let in_depth = items_of(&support::in_depth_chapter());
    let chapter_documents = [intuition[0].payload.doc_id, in_depth[0].payload.doc_id];
    // Looked at before anything is paid for, so that a missing key or a missing picture costs
    // nothing.
    let embedder =
        GeminiEmbedder::from_config(config).expect("EMBEDDING_GEMINI_API_KEY should be set");
    assert!(
        support::sample_picture().is_file(),
        "the sample picture is missing: {:?}",
        support::sample_picture()
    );

    let first = throwaway.rag_ingest_asking_claude([support::intuition_chapter()]);
    print_output("the intuition chapter", &first);
    stop_unless_ingested(config, "the intuition chapter", &first);
    let second = throwaway.rag_ingest_asking_claude([support::in_depth_chapter()]);
    print_output("the in-depth chapter", &second);
    stop_unless_ingested(config, "the in-depth chapter", &second);

    // What the two chapters made is printed and checked before the chart is started, so that it
    // is not lost when the chart fails or the run is cut short. The log comes first, because it
    // is read from a file and needs no store.
    let chapter_log = decisions_in(config);
    print_decisions("of the two chapters", &chapter_log);
    let graph = FalkorGraph::connect(config).await.unwrap();
    let after_chapters = stored_concept_graph(&graph).await;
    println!(
        "--- the concepts after the two chapters\n{}",
        describe_concepts(&after_chapters)
    );
    println!("--- the Black–Scholes links in the decision log");
    for model in black_scholes_models(&after_chapters) {
        for mention in after_chapters
            .mentions
            .iter()
            .filter(|mention| mention.concept == model.normalised_name)
        {
            match decision_for_mention(&chapter_log, &mention.item, &model.id) {
                Some(line) => println!("{line}"),
                None => println!("NO LINE for item {} and {}", mention.item, model.name),
            }
        }
    }
    let mut faults = model_faults(
        &after_chapters,
        &chapter_log,
        [
            ("intuition", intuition.as_slice()),
            ("in-depth", in_depth.as_slice()),
        ],
    );
    println!("--- what is wrong after the two chapters: {faults:?}");

    let chart_run = throwaway.rag_ingest_asking_claude([
        support::sample_picture().into_os_string(),
        OsString::from("--note"),
        OsString::from(NOTE),
    ]);
    print_output("the chart", &chart_run);
    let after_chart = stored_concept_graph(&graph).await;
    let log = decisions_in(config);
    let points = points_in(config).await;
    let chart_point = points
        .iter()
        .find(|(_, payload)| payload["doc_title"] == CHART_TITLE);
    let chart_item = chart_point.map(|(id, _)| id.as_str()).unwrap_or_default();
    let chart_mentions = mentions_of(&after_chart, chart_item);
    let ids_after_chapters: BTreeSet<&str> = after_chapters
        .concepts
        .iter()
        .map(|concept| concept.id.as_str())
        .collect();
    let existed_before = |normalised_name: &str| {
        concept_named(&after_chart, normalised_name)
            .is_some_and(|concept| ids_after_chapters.contains(concept.id.as_str()))
    };
    println!("--- the links of the chart");
    for mention in &chart_mentions {
        let known = if existed_before(&mention.concept) {
            "existed before"
        } else {
            "is new"
        };
        println!(
            "{:?} -> {} ({known}; wording {:?})",
            mention.item, mention.concept, mention.wording
        );
    }
    print_decisions(
        "of the chart",
        log.get(chapter_log.len()..).unwrap_or_default(),
    );
    let ended_well: Vec<&Output> = [&first, &second, &chart_run]
        .into_iter()
        .filter(|output| output.status.success())
        .collect();
    let total_calls: usize = ended_well
        .iter()
        .map(|output| calls_and_cache_hits(&stdout_of(output)).0)
        .sum();
    println!(
        "--- claude calls made by the {} of 3 commands that ended well: {total_calls}",
        ended_well.len()
    );
    // The search comes last of what is printed, because it is one more call that can fail.
    let hits = nearest_to_question(config, &embedder)
        .await
        .unwrap_or_else(|error| {
            println!("--- the search failed: {error:#}");
            Vec::new()
        });
    println!("--- the five nearest items to the question {QUESTION:?}");
    for hit in &hits {
        let payload = &hit.payload;
        println!(
            "{:.3} {} {} page {}: {}",
            hit.score,
            payload.kind.as_str(),
            payload.doc_title,
            payload.page,
            payload.text.chars().take(120).collect::<String>()
        );
    }

    // Everything is printed before the test can fail on a result, and every check is made, so that
    // one paid run says all that is wrong.
    if !chart_run.status.success() {
        faults.push(format!(
            "the chart was not ingested: {}",
            stderr_of(&chart_run)
        ));
    }
    match chart_point {
        Some((_, payload)) => {
            let ends_with_the_note = payload["text"]
                .as_str()
                .is_some_and(|text| text.ends_with(NOTE));
            if !ends_with_the_note {
                faults.push(format!(
                    "the stored text of the chart does not end with the note: {payload}"
                ));
            }
        }
        None => faults.push("the collection holds no point for the chart".to_owned()),
    }
    if !chart_mentions
        .iter()
        .any(|mention| existed_before(&mention.concept))
    {
        faults.push(format!(
            "the chart mentions no concept that the chapters already held: {chart_mentions:?}"
        ));
    }
    let chart_is_found = hits.iter().any(|hit| {
        hit.payload.kind == ItemKind::Figure
            && hit.payload.doc_title == CHART_TITLE
            && hit.payload.image_path.is_some()
    });
    if !chart_is_found {
        faults.push("the chart is not among the five nearest items to the question".to_owned());
    }
    let chapter_text_is_found = hits.iter().any(|hit| {
        hit.payload.kind == ItemKind::Chunk && chapter_documents.contains(&hit.payload.doc_id)
    });
    if !chapter_text_is_found {
        faults.push(
            "no text of the two chapters is among the five nearest items to the question"
                .to_owned(),
        );
    }
    assert!(
        faults.is_empty(),
        "{} checks failed:\n{}",
        faults.len(),
        faults.join("\n")
    );
}
