//! Checks the `export_insights` tool over the committed sample chapters, copied to a temporary
//! folder: the three kinds of file it writes, and every refusal it makes before it writes one.

use std::path::Path;

use ocr::{ChapterInsights, Insight};
use serde_json::json;

use crate::support::{
    ClosedPorts, call, connect, copy_samples, error_text, sample_notes_document_id, structured,
};

const TAKEAWAY: &str = "A call is worth more when volatility rises.";

fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test(flavor = "multi_thread")]
async fn export_insights_writes_each_format_and_refuses_what_it_must() {
    let ports = ClosedPorts::new();
    let content = ports.config.content_folder.clone();
    copy_samples(&content);
    ChapterInsights {
        model: "claude-sonnet-5-5".to_owned(),
        prompt_version: "1".to_owned(),
        insights: vec![Insight {
            text: "The price is $S N(d_1) - K e^{-rT} N(d_2)$.".to_owned(),
            pages: vec![1, 3],
        }],
        takeaways: vec![TAKEAWAY.to_owned()],
        cheat_sheet: "### Terms\n\n| a | b |\n|---|---|\n| 1 | 2 |".to_owned(),
    }
    .write(&content.join("quanty-sample-notes/chapter-2"))
    .unwrap();
    let out = ports.folder().join("exports");
    std::fs::create_dir(&out).unwrap();
    let client = connect(ports.server()).await;
    let saved = sample_notes_document_id(2).to_string();
    let unwritten = sample_notes_document_id(1).to_string();

    let markdown = out.join("greeks.md");
    let result = call(
        &client,
        "export_insights",
        json!({ "document_id": saved, "format": "markdown", "path": markdown }),
    )
    .await;

    assert_eq!(
        structured(&result),
        &json!({
            "document_id": saved,
            "format": "markdown",
            "path": markdown.display().to_string(),
            "bytes": std::fs::metadata(&markdown).unwrap().len(),
        })
    );
    let text = std::fs::read_to_string(&markdown).unwrap();
    assert!(text.contains(TAKEAWAY), "{text}");

    let html = out.join("greeks.html");
    let result = call(
        &client,
        "export_insights",
        json!({ "document_id": saved, "format": "HTML", "path": html }),
    )
    .await;

    let written = structured(&result);
    assert_eq!(written["format"], "html");
    assert_eq!(written["bytes"], std::fs::metadata(&html).unwrap().len());
    let page = std::fs::read_to_string(&html).unwrap();
    assert!(page.starts_with("<!DOCTYPE html>"), "{page:.80}");

    let pdf = out.join("greeks.pdf");
    let result = call(
        &client,
        "export_insights",
        json!({ "document_id": saved, "format": "pdf", "path": pdf }),
    )
    .await;

    let written = structured(&result);
    assert_eq!(written["format"], "pdf");
    assert_eq!(written["bytes"], std::fs::metadata(&pdf).unwrap().len());
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));

    std::fs::write(&markdown, "mine").unwrap();
    let refused = call(
        &client,
        "export_insights",
        json!({ "document_id": saved, "format": "markdown", "path": markdown }),
    )
    .await;

    let text = error_text(&refused);
    assert!(text.contains("is there already"), "{text}");
    assert!(text.contains("`overwrite: true`"), "{text}");
    assert_eq!(std::fs::read_to_string(&markdown).unwrap(), "mine");

    let replaced = call(
        &client,
        "export_insights",
        json!({ "document_id": saved, "format": "markdown", "path": markdown, "overwrite": true }),
    )
    .await;

    let written = structured(&replaced);
    assert_eq!(
        written["bytes"],
        std::fs::metadata(&markdown).unwrap().len()
    );
    assert!(
        std::fs::read_to_string(&markdown)
            .unwrap()
            .contains(TAKEAWAY)
    );

    let relative = "exported-by-the-relative-path-test.md";
    let refusals = [
        (
            json!({ "document_id": saved, "format": "markdown", "path": relative }),
            "must be an absolute path",
        ),
        (
            json!({ "document_id": saved, "format": "markdown", "path": out.join("greeks.txt") }),
            "must end with .md",
        ),
        (
            json!({ "document_id": saved, "format": "markdown", "path": out.join("no/such/greeks.md") }),
            "does not exist",
        ),
        (
            json!({ "document_id": saved, "format": "docx", "path": out.join("greeks.docx") }),
            "`format` must be `pdf`, `markdown` or `html`",
        ),
        (
            json!({ "document_id": unwritten, "format": "markdown", "path": out.join("none.md") }),
            "`write_insights`",
        ),
    ];
    for (arguments, words) in refusals {
        let refused = call(&client, "export_insights", arguments).await;

        let text = error_text(&refused);
        assert!(text.contains(words), "{words}: {text}");
    }
    assert_eq!(
        names_in(&out),
        ["greeks.html", "greeks.md", "greeks.pdf"],
        "a refused call writes nothing"
    );
    assert!(!Path::new(relative).exists());
}
