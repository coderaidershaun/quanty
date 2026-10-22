//! Checks the insights tools over the committed sample chapters, copied to a temporary folder so
//! that no test writes into the samples.

use std::path::PathBuf;

use ocr::{ChapterIndex, ChapterInsights, DocumentName, Insight};
use rag_ingestion::testing::StandInLlm;
use serde_json::json;

use crate::support::{
    ClosedPorts, Library, StandInServices, call, connect, copy_samples, error_text,
    report_when_ended, sample_document_id, sample_notes_document_id, structured,
};

#[tokio::test(flavor = "multi_thread")]
async fn chapter_insights_reads_the_saved_file_and_says_when_there_is_none() {
    let ports = ClosedPorts::new();
    let content = ports.config.content_folder.clone();
    copy_samples(&content);
    let notes = content.join("quanty-sample-notes");
    let saved = ChapterInsights {
        model: "claude-sonnet-5-5".to_owned(),
        prompt_version: "1".to_owned(),
        insights: vec![Insight {
            text: "The price is $S N(d_1) - K e^{-rT} N(d_2)$.".to_owned(),
            pages: vec![1, 3],
        }],
        takeaways: vec!["A call is worth more when volatility rises.".to_owned()],
        cheat_sheet: "### Terms\n\n| a | b |\n|---|---|\n| 1 | 2 |".to_owned(),
    };
    saved.write(&notes.join("chapter-2")).unwrap();
    let client = connect(ports.server()).await;
    let chapter_2 = sample_notes_document_id(2).to_string();

    let result = call(
        &client,
        "chapter_insights",
        json!({ "document_id": chapter_2 }),
    )
    .await;

    assert_eq!(
        structured(&result),
        &json!({
            "document_id": chapter_2,
            "media": "Quanty Sample Notes",
            "chapter_number": 2,
            "chapter_name": "Black Scholes In Depth",
            "pages": 3,
            "state": "saved",
            "file": notes.join("chapter-2/insights.json").display().to_string(),
            "model": "claude-sonnet-5-5",
            "prompt_version": "1",
            "insights": [{
                "text": "The price is $S N(d_1) - K e^{-rT} N(d_2)$.",
                "pages": [1, 3],
            }],
            "takeaways": ["A call is worth more when volatility rises."],
            "cheat_sheet": "### Terms\n\n| a | b |\n|---|---|\n| 1 | 2 |",
        })
    );

    let chapter_1 = sample_notes_document_id(1).to_string();
    let result = call(
        &client,
        "chapter_insights",
        json!({ "document_id": chapter_1 }),
    )
    .await;

    assert_eq!(
        structured(&result),
        &json!({
            "document_id": chapter_1,
            "media": "Quanty Sample Notes",
            "chapter_number": 1,
            "chapter_name": "Options Pricing Intuition",
            "pages": 3,
            "state": "not_written_yet",
            "insights": [],
            "takeaways": [],
        })
    );

    let broken = notes.join("chapter-1/insights.json");
    std::fs::write(&broken, "not json").unwrap();

    let result = call(
        &client,
        "chapter_insights",
        json!({ "document_id": chapter_1 }),
    )
    .await;

    let text = error_text(&result);
    assert!(text.contains(&broken.display().to_string()), "{text}");
    assert!(text.contains("`write_insights`"), "{text}");
}

/// The copy of the sample chapters that a test may write into, in the folder of the stores.
fn copied_samples(library: &Library) -> PathBuf {
    let content = library.stores.temporary_folder().join("content-copy");
    copy_samples(&content);
    content
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored insights::"]
async fn write_insights_writes_the_missing_files_and_a_second_run_asks_nothing() {
    let library = Library::ingested("mcp-insights-write").await;
    let content = copied_samples(&library);
    let llm = StandInLlm::finding_nothing();
    let client =
        connect(library.server_over(&content, StandInServices::new().with_llm(llm.clone()))).await;
    let sample = content.join("option-volatility-and-pricing/chapter-1");
    let titles = [
        "Option Volatility and Pricing, chapter 1: Sample Pages",
        "Quanty Sample Notes, chapter 1: Options Pricing Intuition",
        "Quanty Sample Notes, chapter 2: Black Scholes In Depth",
    ];

    let dry = call(
        &client,
        "write_insights",
        json!({ "all": true, "dry_run": true }),
    )
    .await;

    let dry = structured(&dry);
    assert_eq!(dry["state"], "done", "{dry}");
    assert_eq!(dry["dry_run"], true);
    assert_eq!(
        (dry["done"].as_u64(), dry["total"].as_u64()),
        (Some(3), Some(3))
    );
    let listed: Vec<(&str, &str)> = dry["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|document| {
            (
                document["title"].as_str().unwrap(),
                document["outcome"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(listed, titles.map(|title| (title, "would_write")));
    assert_eq!(dry["usage"], json!([]));
    assert_eq!(dry["cost_usd"], 0.0);
    assert_eq!(llm.calls(), 0, "a dry run asks no model");
    assert!(!sample.join("insights.json").exists());

    let one = call(
        &client,
        "write_insights",
        json!({ "document_id": sample_document_id().to_string() }),
    )
    .await;

    let one = report_when_ended(
        &client,
        "insights_status",
        structured(&one)["job_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(one["state"], "done", "{one}");
    assert_eq!(one["dry_run"], false);
    let written = &one["documents"][0];
    assert_eq!(written["outcome"], "written", "{one}");
    assert_eq!(
        written["file"],
        sample.join("insights.json").display().to_string()
    );
    assert!(sample.join("insights.json").is_file());
    assert!(one.get("current").is_none(), "{one}");
    assert_eq!(one["cost_usd"], 0.01);
    assert_eq!(llm.calls(), 1);
    let read = call(
        &client,
        "chapter_insights",
        json!({ "document_id": sample_document_id().to_string() }),
    )
    .await;
    let read = structured(&read);
    assert_eq!(read["state"], "saved");
    assert_eq!(read["model"], "stand-in");
    assert_eq!(
        read["takeaways"],
        json!(["An option's value rises with volatility."])
    );

    let media = call(
        &client,
        "write_insights",
        json!({ "media": "QUANTY SAMPLE NOTES" }),
    )
    .await;

    let media = report_when_ended(
        &client,
        "insights_status",
        structured(&media)["job_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(media["state"], "done", "{media}");
    assert_eq!(media["total"], 2);
    let outcomes: Vec<&str> = media["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|document| document["outcome"].as_str().unwrap())
        .collect();
    assert_eq!(outcomes, ["written", "written"]);
    assert_eq!(llm.calls(), 3);

    let again = call(&client, "write_insights", json!({ "all": true })).await;

    let again = report_when_ended(
        &client,
        "insights_status",
        structured(&again)["job_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(again["state"], "done", "{again}");
    let outcomes: Vec<&str> = again["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|document| document["outcome"].as_str().unwrap())
        .collect();
    assert_eq!(outcomes, ["already_saved"; 3]);
    assert_eq!(llm.calls(), 3, "a second run asks nothing");
    assert_eq!(again["usage"], json!([]));
    assert_eq!(again["cost_usd"], 0.0);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored insights::"]
async fn a_part_of_a_book_gets_no_insights_and_every_tool_says_so() {
    let library = Library::ingested("mcp-insights-part").await;
    let content = copied_samples(&library);
    let folder = content.join("option-volatility-and-pricing/chapter-1");
    let mut index = ChapterIndex::read(&folder).unwrap();
    index.name = DocumentName::Title("Option Volatility and Pricing, Index".to_owned());
    index.write(&folder).unwrap();
    let llm = StandInLlm::finding_nothing();
    let client =
        connect(library.server_over(&content, StandInServices::new().with_llm(llm.clone()))).await;
    let id = sample_document_id().to_string();

    let started = call(&client, "write_insights", json!({ "document_id": id })).await;

    let ended = report_when_ended(
        &client,
        "insights_status",
        structured(&started)["job_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(ended["state"], "done", "{ended}");
    assert_eq!(
        ended["documents"][0]["outcome"], "none_for_book_part",
        "{ended}"
    );
    assert_eq!(llm.calls(), 0, "no question is asked for a part of a book");
    assert!(!folder.join("insights.json").exists());

    let read = call(&client, "chapter_insights", json!({ "document_id": id })).await;

    let read = structured(&read);
    assert_eq!(read["state"], "none_for_book_part", "{read}");
    assert_eq!(
        read["document_title"],
        "Option Volatility and Pricing, Index"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored insights::"]
async fn a_second_insights_job_is_refused_and_stop_job_stops_the_first() {
    let library = Library::ingested("mcp-insights-stop").await;
    let content = copied_samples(&library);
    let (services, open_the_gate) = StandInServices::new().with_question_gate();
    let client = connect(library.server_over(&content, services)).await;
    let held = content.join("option-volatility-and-pricing/chapter-1");

    let first = call(&client, "write_insights", json!({ "all": true })).await;

    let started = structured(&first);
    assert_eq!(started["state"], "running", "{started}");
    assert_eq!(
        started["current"],
        "Option Volatility and Pricing, chapter 1: Sample Pages"
    );
    let job_id = started["job_id"].as_str().unwrap();
    let second = call(&client, "write_insights", json!({ "all": true })).await;
    let text = error_text(&second);
    assert!(text.contains(job_id), "{text}");
    assert!(text.contains("insights_status"), "{text}");

    let stopped = call(&client, "stop_job", json!({ "job_id": job_id })).await;

    assert_eq!(
        structured(&stopped),
        &json!({
            "job_id": job_id,
            "kind": "insights",
            "state": "stopped",
            "status_tool": "insights_status",
        })
    );
    let status = call(&client, "insights_status", json!({ "job_id": job_id })).await;
    let report = structured(&status);
    assert_eq!(report["state"], "stopped", "{report}");
    assert!(report.get("current").is_none(), "{report}");
    let error = report["error"].as_str().unwrap();
    assert!(error.starts_with("stopped by `stop_job`"), "{error}");
    assert!(error.contains("call `write_insights` again"), "{error}");
    assert!(
        !held.join("insights.json").exists(),
        "the held chapter got none"
    );

    let third = call(&client, "write_insights", json!({ "all": true })).await;

    let restarted = structured(&third);
    assert_eq!(restarted["state"], "running", "{restarted}");
    assert_ne!(restarted["job_id"], started["job_id"], "a job of its own");
    for _ in 0..3 {
        open_the_gate.send(()).unwrap();
    }
    let ended = report_when_ended(
        &client,
        "insights_status",
        restarted["job_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(ended["state"], "done", "{ended}");
    assert!(held.join("insights.json").is_file());
}
