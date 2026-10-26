//! Checks `chapter_insights` over the committed sample chapters, copied to a temporary folder so
//! that no test writes into the samples.

use ocr::{ChapterInsights, Insight};
use serde_json::json;

use crate::support::{
    ClosedPorts, call, connect, copy_samples, error_text, sample_notes_document_id, structured,
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
