//! Checks `search` and `answer` against throwaway stores that hold the three committed chapters,
//! with an embedder and a model that are stand-ins, so nothing is billed.

use serde_json::{Value, json};

use crate::support::{
    Library, QUESTION, call, connect, sample_chapter_folder, sample_document_id, structured,
};

const SAMPLE_TITLE: &str = "Option Volatility and Pricing, chapter 1: Sample Pages";

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored retrieval::"]
async fn search_gives_the_placed_item_first_and_explain_says_how_it_was_found() {
    let library = Library::ingested("mcp-search").await;
    let client = connect(library.server()).await;

    let result = call(&client, "search", json!({ "question": QUESTION })).await;

    let found = structured(&result);
    assert!(found.get("trace").is_none(), "a trace only with explain");
    let results = found["results"].as_array().unwrap();
    let first = &results[0];
    assert_eq!(first["number"], 1);
    assert_eq!(first["document_id"], sample_document_id().to_string());
    assert_eq!(first["document"], SAMPLE_TITLE);
    assert_eq!(first["book"], "Option Volatility and Pricing");
    assert_eq!(first["page"], 5);
    assert_eq!(first["printed_page"], "233");
    assert_eq!(first["kind"], "figure");
    assert_eq!(first["label"], "Figure 13-4");
    assert_eq!(first["reason"], json!({ "why": "nearest" }));
    assert!(first["score"].as_f64().unwrap() > 0.99, "{first}");
    assert!(!first["text"].as_str().unwrap().is_empty());
    let picture = std::fs::canonicalize(sample_chapter_folder())
        .unwrap()
        .join("page-num-5/01-figure.png");
    assert_eq!(first["picture"], picture.display().to_string());
    let numbers: Vec<u64> = results
        .iter()
        .map(|result| result["number"].as_u64().unwrap())
        .collect();
    assert_eq!(numbers, (1..=results.len() as u64).collect::<Vec<_>>());

    let result = call(
        &client,
        "search",
        json!({
            "question": QUESTION,
            "limit": 2,
            "book": "option volatility and pricing",
            "explain": true,
        }),
    )
    .await;

    let found = structured(&result);
    let results = found["results"].as_array().unwrap();
    assert_eq!(results.len(), 2, "the limit cuts the list");
    assert!(
        results
            .iter()
            .all(|result| result["book"] == "Option Volatility and Pricing"),
        "only items of that book"
    );
    let trace = &found["trace"];
    assert_eq!(trace["documents_searched"], 1);
    assert!(trace["seeds"].as_u64().unwrap() > 0, "{trace}");
    assert!(trace["candidates"].as_u64().unwrap() >= trace["seeds"].as_u64().unwrap());
    assert!(trace["ranked"].as_u64().unwrap() > 0);
    assert!(
        trace["kept"].as_u64().unwrap() > 2,
        "the search kept more than the limit cut: {trace}"
    );
    assert!(trace["question_concepts"].is_array());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored retrieval::"]
async fn answer_gives_claims_with_the_results_they_rest_on() {
    let library = Library::ingested("mcp-answer").await;
    let client = connect(library.server()).await;
    let calls_before = library.llm.calls();

    let result = call(&client, "answer", json!({ "question": QUESTION })).await;

    let answer = structured(&result);
    assert_eq!(answer["answered"], true);
    assert_eq!(answer["title"], "What the three spreads share");
    assert_eq!(answer["follow_ups"], json!(["What is a long butterfly?"]));
    let claims = answer["claims"].as_array().unwrap();
    assert_eq!(claims.len(), 2);
    assert_eq!(claims[0]["heading"], "The chart");
    assert_eq!(claims[0]["sources"], json!([1]));
    assert!(claims[1].get("heading").is_none());
    assert_eq!(claims[1]["sources"], json!([1, 2]));
    let sources = answer["sources"].as_array().unwrap();
    let numbers: Vec<&Value> = sources.iter().map(|source| &source["number"]).collect();
    assert_eq!(numbers, [1, 2], "each source once, by number");
    assert_eq!(sources[0]["document"], SAMPLE_TITLE);
    assert_eq!(sources[0]["document_id"], sample_document_id().to_string());
    assert_eq!(sources[0]["page"], 5);
    assert_eq!(sources[0]["kind"], "figure");
    assert_eq!(
        library.llm.calls(),
        calls_before + 1,
        "one question to the model"
    );

    let result = call(
        &client,
        "answer",
        json!({ "question": QUESTION, "book": "A Book That Nobody Has" }),
    )
    .await;

    let nothing = structured(&result);
    assert_eq!(nothing["answered"], false);
    assert_eq!(nothing["claims"], json!([]));
    assert_eq!(
        library.llm.calls(),
        calls_before + 1,
        "when nothing is found the model is not asked, so no Claude usage is spent"
    );
}
