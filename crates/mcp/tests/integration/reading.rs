//! Checks the tools that read a converted chapter as a whole, over the committed sample chapters
//! with the stores down: the overview, the outline, a section, a run of pages, an item, and reads
//! that are cut by a budget of characters.

use std::fs;

use ocr::ChapterInsights;
use serde_json::{Value, json};

use crate::support::{
    Client, ClosedPorts, call, chars, connect, counted, dataset_sections, error_text, headings_of,
    listed_and_read_back_items, long_notes_copy, parts_of, sample_chapter_folder,
    sample_content_folder, sample_document_id, sample_notes_document_id, structured,
};

#[tokio::test(flavor = "multi_thread")]
async fn the_sample_chapters_are_read_by_outline_section_page_and_item_within_a_budget() {
    let ports = ClosedPorts::new().with_content_folder(&sample_content_folder());
    let client = connect(ports.server()).await;

    an_overview_gives_the_notes_and_names_the_graph_that_is_down(&client).await;
    the_outline_covers_the_pages_that_page_json_lists(&client).await;
    a_section_reads_as_the_dataset_renders_it_and_as_pieces(&client).await;
    a_long_chapter_has_sections_that_are_found_by_their_heading().await;
    reads_cut_by_a_budget_put_together_are_the_whole_text(&client).await;
    read_many_shares_one_budget(&client).await;
    an_item_is_read_by_label_and_by_id_with_the_paragraphs_that_cite_it(&client).await;
}

async fn an_overview_gives_the_notes_and_names_the_graph_that_is_down(client: &Client) {
    let notes = sample_notes_document_id(2).to_string();
    let saved =
        ChapterInsights::read(&sample_content_folder().join("quanty-sample-notes/chapter-2"))
            .unwrap()
            .expect("the notes chapter has saved insights");

    let full = call(
        client,
        "document_overview",
        json!({ "document_id": notes, "detail": "full" }),
    )
    .await;

    let full = structured(&full);
    assert_eq!(full["document_id"], notes.as_str());
    assert_eq!(full["pages"], 3);
    assert_eq!(
        full["items"],
        json!({ "formula": 6, "figure": 0, "table": 0, "code": 1 })
    );
    assert_eq!(full["insights"]["state"], "saved");
    assert_eq!(full["insights"]["cheat_sheet"], saved.cheat_sheet.as_str());
    assert_eq!(full["insights"]["takeaways"], json!(saved.takeaways));
    assert_eq!(
        full["insights"]["insights"].as_array().unwrap().len(),
        saved.insights.len()
    );
    assert!(!full["outline"].as_array().unwrap().is_empty());
    assert!(full["artifacts"]["state"].is_string());
    assert!(
        full["concepts"]["error"].is_string() && full["related_documents"]["error"].is_string(),
        "the graph is down, so the concepts are an error and the rest is given: {full}"
    );
    assert!(full.get("ingested_whole").is_none());
    let outline_rows = full["outline"].as_array().unwrap();
    assert!(
        outline_rows.iter().all(|row| row.get("concepts").is_none()),
        "no section counts concepts when the graph is down"
    );

    let summary = call(client, "document_overview", json!({ "document_id": notes })).await;

    let summary = structured(&summary);
    assert_eq!(summary["insights"]["state"], "saved");
    assert_eq!(summary["insights"]["takeaways"], json!(saved.takeaways));
    assert!(summary["insights"].get("cheat_sheet").is_none());
    assert!(summary["insights"].get("insights").is_none());
    assert!(summary.get("outline").is_none() && summary.get("related_documents").is_none());
    assert!(summary.to_string().len() < full.to_string().len());
}

async fn the_outline_covers_the_pages_that_page_json_lists(client: &Client) {
    let chapters = [
        (sample_document_id(), sample_chapter_folder(), 7),
        (
            sample_notes_document_id(2),
            sample_content_folder().join("quanty-sample-notes/chapter-2"),
            3,
        ),
    ];
    for (document, folder, pages) in chapters {
        let outline = call(
            client,
            "document_outline",
            json!({ "document_id": document.to_string() }),
        )
        .await;

        let outline = structured(&outline);
        let rows = outline["sections"].as_array().unwrap();
        assert!(!rows.is_empty());
        assert_eq!(rows[0]["pages"][0], 1);
        assert_eq!(rows.last().unwrap()["pages"][1], pages);
        let headings = headings_of(&folder, pages);
        let mut previous_start = 0;
        for (place, row) in rows.iter().enumerate() {
            assert_eq!(row["index"], place + 1);
            let (from, to) = (
                row["pages"][0].as_u64().unwrap(),
                row["pages"][1].as_u64().unwrap(),
            );
            assert!(
                previous_start <= from && from <= to && to <= u64::from(pages),
                "{row}"
            );
            previous_start = from;
            let Some(rank) = row.get("rank") else {
                continue;
            };
            let last = row["path"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .as_str()
                .unwrap();
            let printed = row.get("printed_number").and_then(Value::as_str);
            assert!(
                headings.iter().any(|(page, heading_rank, number, text)| {
                    (from..=to).contains(&u64::from(*page))
                        && *heading_rank == rank.as_u64().unwrap()
                        && number.as_deref() == printed
                        && last.ends_with(text.as_str())
                }),
                "no heading of page.json is the heading of {row}"
            );
        }
        let nodes = outline["nodes"].as_array().unwrap();
        assert_eq!(outline["version"], 1);
        assert_eq!(nodes.len(), rows.len());
        assert_eq!(nodes[0]["kind"], "tool");
        assert_eq!(nodes[0]["tool"], "read_section");
        assert_eq!(
            nodes[0]["arguments"],
            json!({ "document_id": document.to_string(), "section": 1 })
        );
        assert!(outline["concepts_error"].is_string());
    }
}

async fn a_section_reads_as_the_dataset_renders_it_and_as_pieces(client: &Client) {
    let document = sample_document_id().to_string();
    let content = sample_content_folder();
    let expected = dataset_sections(&sample_chapter_folder(), &content);

    for (place, section) in expected.iter().enumerate() {
        let index = place + 1;
        let read = call(
            client,
            "read_section",
            json!({ "document_id": document, "section": index }),
        )
        .await;
        let read = structured(&read);
        assert_eq!(read["text"], section.markdown.as_str(), "section {index}");
        assert_eq!(read["index"], index);
        assert_eq!(read["path"], json!(section.section_path));
        assert_eq!(read["truncated"], false);
        assert!(read.get("cursor").is_none());
        let ids: Vec<String> = section.item_ids.iter().map(ToString::to_string).collect();
        assert_eq!(read["item_ids"], json!(ids));

        let pieces = call(
            client,
            "read_section",
            json!({ "document_id": document, "section": index, "as": "pieces" }),
        )
        .await;
        let pieces = structured(&pieces);
        assert!(pieces.get("text").is_none());
        let numbers: Vec<u64> = pieces["pieces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|piece| piece["number"].as_u64().unwrap())
            .collect();
        let expected_numbers: Vec<u64> = section
            .piece_ids
            .iter()
            .map(|id| u64::from(id.number))
            .collect();
        assert_eq!(numbers, expected_numbers, "the pieces of section {index}");
    }

    let past_the_end = call(
        client,
        "read_section",
        json!({ "document_id": document, "section": expected.len() + 1 }),
    )
    .await;
    assert!(error_text(&past_the_end).contains("`document_outline`"));
    let unknown = call(
        client,
        "read_section",
        json!({ "document_id": document, "section": "No such heading" }),
    )
    .await;
    assert!(error_text(&unknown).contains("`document_outline`"));
}

async fn reads_cut_by_a_budget_put_together_are_the_whole_text(client: &Client) {
    let document = sample_document_id().to_string();

    let whole = call(
        client,
        "read_section",
        json!({ "document_id": document, "section": 1 }),
    )
    .await;
    let whole = structured(&whole)["text"].as_str().unwrap().to_owned();
    let parts = parts_of(
        client,
        "read_section",
        json!({ "document_id": document, "section": 1 }),
        97,
    )
    .await;
    assert!(parts.len() > 3);
    assert!(parts.iter().all(|part| chars(&part["text"]) <= 97));
    let joined: String = parts
        .iter()
        .map(|part| part["text"].as_str().unwrap())
        .collect();
    assert_eq!(joined, whole);

    let pages = json!({ "document_id": document, "from": 1, "to": 7 });
    let whole = call(client, "read_pages", pages.clone()).await;
    let whole = structured(&whole)["text"].as_str().unwrap().to_owned();
    let parts = parts_of(client, "read_pages", pages, 211).await;
    let joined: String = parts
        .iter()
        .map(|part| part["text"].as_str().unwrap())
        .collect();
    assert_eq!(joined, whole);

    let page = json!({ "document_id": document, "page": 4 });
    let whole = call(client, "read_page", page.clone()).await;
    let whole: String = structured(&whole)["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|piece| piece["content"].as_str().unwrap())
        .collect();
    let parts = parts_of(client, "read_page", page, 53).await;
    let joined: String = parts
        .iter()
        .flat_map(|part| part["pieces"].as_array().unwrap())
        .map(|piece| piece["content"].as_str().unwrap())
        .collect();
    assert_eq!(joined, whole);

    let first = call(
        client,
        "read_section",
        json!({ "document_id": document, "section": 1, "max_chars": 50 }),
    )
    .await;
    let cursor = structured(&first)["cursor"].as_str().unwrap().to_owned();
    let foreign = call(
        client,
        "read_pages",
        json!({ "document_id": document, "from": 1, "to": 2, "cursor": cursor }),
    )
    .await;
    assert!(error_text(&foreign).contains("`cursor` is not from this read"));
    let garbled = call(
        client,
        "read_page",
        json!({ "document_id": document, "page": 1, "cursor": "not a cursor" }),
    )
    .await;
    assert!(error_text(&garbled).contains("call again without `cursor`"));
    let zero = call(
        client,
        "read_page",
        json!({ "document_id": document, "page": 1, "max_chars": 0 }),
    )
    .await;
    assert!(error_text(&zero).contains("`max_chars`"));
    let huge = call(
        client,
        "read_page",
        json!({ "document_id": document, "page": 1, "max_chars": 200_001 }),
    )
    .await;
    assert!(error_text(&huge).contains("200000"));
}

async fn read_many_shares_one_budget(client: &Client) {
    let document = sample_document_id().to_string();
    let notes = sample_notes_document_id(2).to_string();
    let requests = json!([
        { "tool": "read_item", "args": { "document_id": document, "kind": "figure", "label": "Figure 13-4" } },
        { "tool": "read_section", "args": { "document_id": document, "section": 1 } },
        { "tool": "read_pages", "args": { "document_id": document, "from": 1, "to": 2 } },
        { "tool": "chapter_insights", "args": { "document_id": notes } },
        { "tool": "read_section", "args": { "document_id": "not an id", "section": 1 } },
    ]);

    let all = call(
        client,
        "read_many",
        json!({ "requests": requests, "max_chars": 100_000 }),
    )
    .await;

    let all = structured(&all);
    let results = all["results"].as_array().unwrap();
    assert_eq!(results.len(), 5);
    let mut sum = 0;
    for result in &results[..4] {
        assert_eq!(result["truncated"], false, "{result}");
        sum += counted(result["tool"].as_str().unwrap(), &result["result"]);
    }
    assert_eq!(all["used_chars"], sum);
    assert!(
        results[4]["error"]
            .as_str()
            .unwrap()
            .contains("`document_id` is not valid")
    );
    assert!(results[4].get("result").is_none());

    let budget = counted("read_item", &results[0]["result"]) + 120;
    let small = call(
        client,
        "read_many",
        json!({ "requests": requests, "max_chars": budget }),
    )
    .await;

    let small = structured(&small);
    let results = small["results"].as_array().unwrap();
    assert_eq!(results[0]["truncated"], false);
    assert_eq!(
        results[1]["truncated"], true,
        "the section is cut at what is left"
    );
    assert_eq!(chars(&results[1]["result"]["text"]), 120);
    for starved in &results[2..4] {
        assert_eq!(starved["truncated"], true);
        assert!(starved.get("result").is_none() && starved.get("error").is_none());
    }
    let used: usize = results[..2]
        .iter()
        .map(|result| counted(result["tool"].as_str().unwrap(), &result["result"]))
        .sum();
    assert_eq!(small["used_chars"], used);
    assert_eq!(used, budget);

    let none = call(client, "read_many", json!({ "requests": [] })).await;
    assert!(error_text(&none).contains("from 1 to 20"));
}

async fn an_item_is_read_by_label_and_by_id_with_the_paragraphs_that_cite_it(client: &Client) {
    let document = sample_document_id().to_string();
    let chapter = sample_chapter_folder();

    let by_label = call(
        client,
        "read_item",
        json!({ "document_id": document, "kind": "figure", "label": " Figure 24-12 " }),
    )
    .await;

    let item = structured(&by_label).clone();
    assert_eq!(item["kind"], "figure");
    assert_eq!(item["label"], "Figure 24-12");
    assert_eq!(item["page"], 7);
    assert_eq!(item["printed_page"], "500");
    assert_eq!(
        item["caption"],
        "FTSE 100 volatility surface. March 16. 2012."
    );
    assert!(chars(&item["content"]) > 0);
    assert!(item["picture"].is_string());
    let cited_text = fs::read_to_string(chapter.join("page-num-7/03-text.md")).unwrap();
    let cited_text = cited_text.trim();
    let excerpt: String = cited_text.chars().take(200).collect();
    let excerpt = if cited_text.chars().count() > 200 {
        format!("{excerpt}…")
    } else {
        excerpt
    };
    assert_eq!(
        item["cited_by"],
        json!([{ "page": 7, "printed_page": "500", "number": 3, "excerpt": excerpt }])
    );
    let item_id = item["item_id"].as_str().unwrap().to_owned();
    let section_index = item["section_index"].as_u64().unwrap();
    let section = call(
        client,
        "read_section",
        json!({ "document_id": document, "section": section_index }),
    )
    .await;
    assert!(
        structured(&section)["item_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(item_id))
    );
    let path = structured(&section)["path"].as_array().unwrap().len();
    assert_eq!(item["section"].as_array().unwrap().len(), 2);
    assert!(
        path <= 2,
        "the headings of the item run on from the path of its section"
    );

    let by_id = call(client, "read_item", json!({ "item_id": item_id })).await;
    assert_eq!(structured(&by_id), &item);
    let narrowed = call(
        client,
        "read_item",
        json!({ "item_id": item_id, "document_id": document }),
    )
    .await;
    assert_eq!(structured(&narrowed), &item);

    // Every stored item of every section reads back as the item the chapter lists.
    let (mut listed, mut found) = listed_and_read_back_items(client, &document).await;
    listed.sort();
    found.sort();
    assert_eq!(found, listed);

    let missing = call(
        client,
        "read_item",
        json!({ "document_id": document, "kind": "figure", "label": "Figure 99-9" }),
    )
    .await;
    assert!(error_text(&missing).contains("`chapter_contents`"));
    let neither = call(client, "read_item", json!({ "document_id": document })).await;
    assert!(error_text(&neither).contains("`item_id`"));
}

/// The first chapter of the sample notes with every paragraph made long, in a copy, so that the
/// chapter is cut into several sections.
async fn a_long_chapter_has_sections_that_are_found_by_their_heading() {
    let ports = long_notes_copy();
    let client = connect(ports.server()).await;
    let document = sample_notes_document_id(1).to_string();

    let outline = call(
        &client,
        "document_outline",
        json!({ "document_id": document }),
    )
    .await;

    let rows = structured(&outline)["sections"].as_array().unwrap().clone();
    assert!(rows.len() >= 3, "{rows:?}");
    let last = rows.last().unwrap();
    let heading = last["path"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_str()
        .unwrap();
    for name in [
        heading.to_uppercase(),
        last["path"]
            .as_array()
            .unwrap()
            .iter()
            .map(|part| part.as_str().unwrap())
            .collect::<Vec<_>>()
            .join(" › "),
    ] {
        let read = call(
            &client,
            "read_section",
            json!({ "document_id": document, "section": name }),
        )
        .await;
        assert_eq!(structured(&read)["index"], last["index"], "{name}");
    }
}
