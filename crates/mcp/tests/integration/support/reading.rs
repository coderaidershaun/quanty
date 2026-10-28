//! What the reading tests compare the tools with: the headings and sections of a sample chapter as
//! its files and the dataset build give them, and reads that follow every cursor.

use std::fs;
use std::path::Path;

use ocr::{Catalogue, read_chapter};
use rag_dataset::documents::{DocRole, ExportDocument, Split};
use rag_dataset::section::{Section, sections_of};
use rag_ingestion::{chapter_items, document_title};
use serde_json::{Value, json};

use super::{Client, ClosedPorts, call, copy_samples, structured};

pub fn chars(value: &Value) -> usize {
    value.as_str().map_or(0, |text| text.chars().count())
}

/// The headings of a chapter as `page.json` and the heading files list them: for each, the page,
/// the rank, the printed number and the text.
pub fn headings_of(chapter: &Path, pages: u32) -> Vec<(u32, u64, Option<String>, String)> {
    let mut headings = Vec::new();
    for page in 1..=pages {
        let folder = chapter.join(format!("page-num-{page}"));
        let index: Value =
            serde_json::from_str(&fs::read_to_string(folder.join("page.json")).unwrap()).unwrap();
        for piece in index["pieces"].as_array().unwrap() {
            if piece["kind"] == "heading" {
                let text =
                    fs::read_to_string(folder.join(piece["file"].as_str().unwrap())).unwrap();
                headings.push((
                    page,
                    piece["rank"].as_u64().unwrap(),
                    piece["printed-number"].as_str().map(str::to_owned),
                    text.trim().to_owned(),
                ));
            }
        }
    }
    headings
}

/// The sections of a chapter as a dataset build cuts them.
pub fn dataset_sections(folder: &Path, content: &Path) -> Vec<Section> {
    let chapter = read_chapter(folder).unwrap();
    let catalogue = Catalogue::read(content).unwrap();
    assert!(
        catalogue
            .chapters
            .iter()
            .any(|entry| entry.folder == folder)
    );
    let document = ExportDocument {
        id: rag_core::DocId::from_source_sha256(&chapter.index.source_sha256),
        title: document_title(&chapter.index),
        media: chapter.index.media_title.clone(),
        folder: folder.to_path_buf(),
        role: DocRole::Chapter,
        split: Split::Train,
    };
    sections_of(&document, &chapter, &chapter_items(&chapter))
}

/// Reads with `max_chars` and follows the cursors, and gives the texts of the parts in order.
pub async fn parts_of(
    client: &Client,
    tool: &'static str,
    arguments: Value,
    max_chars: u32,
) -> Vec<Value> {
    let mut parts = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut arguments = arguments.clone();
        arguments["max_chars"] = json!(max_chars);
        if let Some(cursor) = &cursor {
            arguments["cursor"] = json!(cursor);
        }
        let result = call(client, tool, arguments).await;
        let part = structured(&result).clone();
        assert!(parts.len() < 500, "the cursors never end");
        cursor = part
            .get("cursor")
            .and_then(Value::as_str)
            .map(str::to_owned);
        assert_eq!(part["truncated"], cursor.is_some(), "{part}");
        parts.push(part);
        if cursor.is_none() {
            return parts;
        }
    }
}

/// The characters of text that a result of `read_many` holds, counted the way the tool counts.
pub fn counted(tool: &str, result: &Value) -> usize {
    match tool {
        "read_section" | "read_pages" => chars(&result["text"]),
        "read_item" => chars(&result["content"]),
        "chapter_insights" => {
            let insights: usize = result["insights"]
                .as_array()
                .unwrap()
                .iter()
                .map(|insight| chars(&insight["text"]))
                .sum();
            let takeaways: usize = result["takeaways"]
                .as_array()
                .unwrap()
                .iter()
                .map(chars)
                .sum();
            insights + takeaways + chars(&result["cheat_sheet"])
        }
        other => panic!("{other} is not read by read_many"),
    }
}

/// A copy of the sample chapters in which every paragraph of the first chapter of the notes is
/// made long, so that the chapter is cut into several sections.
pub fn long_notes_copy() -> ClosedPorts {
    let ports = ClosedPorts::new();
    copy_samples(&ports.config.content_folder);
    let chapter = ports
        .config
        .content_folder
        .join("quanty-sample-notes/chapter-1");
    for page in 1..=3 {
        for file in fs::read_dir(chapter.join(format!("page-num-{page}"))).unwrap() {
            let path = file.unwrap().path();
            if path.to_string_lossy().ends_with("-text.md") {
                let mut text = fs::read_to_string(&path).unwrap();
                text.push_str(&" A long straddle gains when the price moves.".repeat(100));
                fs::write(&path, text).unwrap();
            }
        }
    }
    ports
}

/// The page and content of every item that `chapter_contents` lists, and of every item that
/// `read_item` gives for an item id of a section.
pub async fn listed_and_read_back_items(
    client: &Client,
    document: &str,
) -> (Vec<(u64, String)>, Vec<(u64, String)>) {
    let pair = |item: &Value| {
        (
            item["page"].as_u64().unwrap(),
            item["content"].as_str().unwrap().to_owned(),
        )
    };
    let contents = call(
        client,
        "chapter_contents",
        json!({ "document_id": document }),
    )
    .await;
    let listed = structured(&contents)["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(pair)
        .collect();
    let outline = call(
        client,
        "document_outline",
        json!({ "document_id": document }),
    )
    .await;
    let sections = structured(&outline)["sections"].as_array().unwrap().len();
    let mut ids: Vec<Value> = Vec::new();
    for index in 1..=sections {
        let section = call(
            client,
            "read_section",
            json!({ "document_id": document, "section": index }),
        )
        .await;
        for id in structured(&section)["item_ids"].as_array().unwrap() {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
    }
    let mut found = Vec::new();
    for id in ids {
        let result = call(client, "read_item", json!({ "item_id": id })).await;
        // A chunk is stored under an item id too, but it is no formula, figure, table or listing.
        if result.is_error != Some(true) {
            found.push(pair(structured(&result)));
        }
    }
    (listed, found)
}
