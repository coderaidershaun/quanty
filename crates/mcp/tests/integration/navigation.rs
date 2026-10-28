//! Checks browsing the stored library: the overview with its concepts, the media and the documents
//! with their filters and pages, and a search that is paged and kept inside one section.

use rag_ingestion::chapter_items;
use serde_json::{Value, json};

use crate::support::{
    AUTHOR, Client, Library, QUESTION, call, connect, error_text, sample_chapter_folder,
    sample_document_id, sample_notes_document_id, structured,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored navigation::"]
async fn the_stored_library_is_browsed_by_media_and_document_and_searched_by_section() {
    let library = Library::ingested_with_concepts("mcp-navigation").await;
    let client = connect(library.server()).await;

    an_overview_names_the_concepts_with_their_mentions(&client).await;
    media_are_filtered_and_named(&client).await;
    documents_are_filtered_named_and_paged(&client).await;
    a_search_is_paged_and_gives_ids_and_sections(&client).await;
    a_search_inside_a_section_finds_only_its_items(&client).await;
}

async fn an_overview_names_the_concepts_with_their_mentions(client: &Client) {
    let sample = sample_document_id().to_string();
    let chapter = ocr::read_chapter(&sample_chapter_folder()).expect("the sample chapter reads");
    let mentioning = chapter_items(&chapter)
        .into_iter()
        .filter(|item| item.input.text.contains("straddle"))
        .count();

    let full = call(
        client,
        "document_overview",
        json!({ "document_id": sample, "detail": "full" }),
    )
    .await;

    let full = structured(&full);
    let concept = &full["concepts"][0];
    assert_eq!(full["concepts"].as_array().unwrap().len(), 1);
    assert_eq!(concept["name"], "Long straddle");
    assert_eq!(concept["mentions"], mentioning);
    assert_eq!(
        concept["definition"],
        "A bet that the price moves a lot, either way."
    );
    assert!(concept["concept_id"].is_string());
    assert_eq!(full["related_documents"], json!([]));
    assert_eq!(full["ingested_whole"], true);
    assert!(full["insights"]["state"].is_string());
    let counts: Vec<u64> = full["outline"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["concepts"].as_u64().expect("the graph is up"))
        .collect();
    assert!(counts.contains(&1));

    let summary = call(
        client,
        "document_overview",
        json!({ "document_id": sample }),
    )
    .await;

    let summary = structured(&summary);
    assert_eq!(summary["concepts"][0]["mentions"], mentioning);
    assert!(summary["concepts"][0].get("definition").is_none());
    assert_eq!(summary["ingested_whole"], true);
    assert!(summary["insights"].get("cheat_sheet").is_none());
    assert!(summary.to_string().len() < full.to_string().len());
}

async fn media_are_filtered_and_named(client: &Client) {
    let all = call(client, "list_media", json!({})).await;

    let media = structured(&all)["media"].as_array().unwrap();
    let titles: Vec<&str> = media
        .iter()
        .map(|row| row["title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        ["Option Volatility and Pricing", "Quanty Sample Notes"]
    );
    assert_eq!(media[0]["category"], "book");
    assert_eq!(media[0]["authors"], json!([AUTHOR]));
    assert_eq!(media[0]["tags"], json!(["options"]));
    assert_eq!(media[0]["documents"], 1);
    assert_eq!(media[0]["pages"], 7);
    assert_eq!(media[0]["items"], 20);
    assert_eq!(media[0]["with_insights"], 0);
    assert_eq!(media[1]["documents"], 2);
    assert_eq!(media[1]["pages"], 6);
    assert_eq!(media[1]["with_insights"], 1);

    let titles_for = async |arguments: Value| {
        let result = call(client, "list_media", arguments).await;
        structured(&result)["media"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["title"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        titles_for(json!({ "author": "sheldon natenberg" })).await,
        ["Option Volatility and Pricing"]
    );
    assert_eq!(
        titles_for(json!({ "tag": "OPTIONS" })).await,
        ["Option Volatility and Pricing"]
    );
    assert_eq!(titles_for(json!({ "category": "book" })).await.len(), 2);
    assert!(titles_for(json!({ "category": "paper" })).await.is_empty());

    let names = call(client, "list_media", json!({ "detail": "names" })).await;

    let names = structured(&names);
    assert_eq!(
        names["media"],
        json!([{ "title": "Option Volatility and Pricing" }, { "title": "Quanty Sample Notes" }])
    );
    assert_eq!(names["version"], 1);
    assert_eq!(
        names["nodes"][1],
        json!({
            "label": "Quanty Sample Notes",
            "kind": "discover",
            "tool": "list_documents",
            "arguments": { "media": "Quanty Sample Notes", "detail": "names" },
        })
    );
}

async fn documents_are_filtered_named_and_paged(client: &Client) {
    let notes = call(
        client,
        "list_documents",
        json!({ "media": "quanty sample NOTES", "detail": "summary" }),
    )
    .await;

    let notes = structured(&notes);
    assert_eq!(notes["total"], 2);
    assert!(notes.get("cursor").is_none());
    let row = &notes["documents"][0];
    let mut keys: Vec<&str> = row
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "category",
            "document_id",
            "ingested_whole",
            "media",
            "pages",
            "title"
        ]
    );

    let by_author = call(
        client,
        "list_documents",
        json!({ "author": AUTHOR, "tags": ["options"] }),
    )
    .await;
    assert_eq!(structured(&by_author)["total"], 1);

    let names = call(
        client,
        "list_documents",
        json!({ "media": "Quanty Sample Notes", "detail": "names" }),
    )
    .await;

    let names = structured(&names);
    let notes_2 = sample_notes_document_id(2).to_string();
    assert_eq!(
        names["documents"][1],
        json!({ "document_id": notes_2, "title": "Quanty Sample Notes, chapter 2: Black Scholes In Depth" })
    );
    assert_eq!(names["version"], 1);
    assert_eq!(
        names["nodes"][1],
        json!({
            "label": "Quanty Sample Notes, chapter 2: Black Scholes In Depth",
            "kind": "discover",
            "tool": "document_outline",
            "arguments": { "document_id": notes_2 },
        })
    );

    let mut titles = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let mut arguments = json!({ "limit": 2, "detail": "names" });
        if let Some(cursor) = &cursor {
            arguments["cursor"] = json!(cursor);
        }
        let page = call(client, "list_documents", arguments).await;
        let page = structured(&page);
        assert_eq!(page["total"], 3);
        pages += 1;
        titles.extend(
            page["documents"]
                .as_array()
                .unwrap()
                .iter()
                .map(|document| document["title"].as_str().unwrap().to_owned()),
        );
        cursor = page
            .get("cursor")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(pages, 2);
    assert_eq!(titles.len(), 3);

    let first = call(client, "list_documents", json!({ "limit": 1 })).await;
    let cursor = structured(&first)["cursor"].as_str().unwrap().to_owned();
    let other = call(
        client,
        "list_documents",
        json!({ "limit": 1, "media": "Quanty Sample Notes", "cursor": cursor }),
    )
    .await;
    assert!(error_text(&other).contains("`cursor` is not from this read"));
    let too_many = call(client, "list_documents", json!({ "limit": 501 })).await;
    assert!(error_text(&too_many).contains("1 to 500"));
}

fn item_ids(result: &Value) -> Vec<String> {
    result["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["item_id"].as_str().unwrap().to_owned())
        .collect()
}

async fn a_search_is_paged_and_gives_ids_and_sections(client: &Client) {
    let full = call(client, "search", json!({ "question": QUESTION })).await;

    let full = structured(&full).clone();
    let all_ids = item_ids(&full);
    assert!(all_ids.len() >= 4, "{full}");
    assert_eq!(full["truncated"], false);
    let first = &full["results"][0];
    assert_eq!(first["kind"], "figure");
    assert!(first["section"]["index"].as_u64().unwrap() >= 1);
    let section = call(
        client,
        "read_section",
        json!({ "document_id": first["document_id"], "section": first["section"]["index"] }),
    )
    .await;
    assert!(
        structured(&section)["item_ids"]
            .as_array()
            .unwrap()
            .contains(&first["item_id"])
    );
    assert_eq!(first["section"]["path"], structured(&section)["path"]);
    let item = call(client, "read_item", json!({ "item_id": first["item_id"] })).await;
    assert_eq!(structured(&item)["content"], first["text"]);

    let ids = call(
        client,
        "search",
        json!({ "question": QUESTION, "detail": "ids" }),
    )
    .await;

    let ids = structured(&ids);
    let row = ids["results"][0].as_object().unwrap();
    let mut keys: Vec<&str> = row.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        ["document_id", "item_id", "kind", "number", "page", "score"]
    );
    assert_eq!(item_ids(ids), all_ids);

    let skipped = call(
        client,
        "search",
        json!({ "question": QUESTION, "offset": 2, "detail": "ids" }),
    )
    .await;
    let skipped = structured(&skipped);
    assert_eq!(skipped["results"][0]["number"], 3);
    assert_eq!(item_ids(skipped), all_ids[2..]);

    let mut paged = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut arguments = json!({ "question": QUESTION, "limit": 2, "detail": "ids" });
        if let Some(cursor) = &cursor {
            arguments["cursor"] = json!(cursor);
        }
        let page = call(client, "search", arguments).await;
        let page = structured(&page);
        assert_eq!(page["results"][0]["number"], paged.len() + 1);
        paged.extend(item_ids(page));
        cursor = page
            .get("cursor")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(paged, all_ids);
    let both = call(
        client,
        "search",
        json!({ "question": QUESTION, "offset": 1, "cursor": "x" }),
    )
    .await;
    assert!(error_text(&both).contains("`offset` or `cursor`, not both"));

    let cut = call(
        client,
        "search",
        json!({ "question": QUESTION, "max_chars": 30 }),
    )
    .await;
    let cut = structured(&cut);
    assert_eq!(cut["truncated"], true);
    assert!(cut["cursor"].is_string());
    assert_eq!(cut["results"].as_array().unwrap().len(), 1);
    assert_eq!(
        cut["results"][0]["text"].as_str().unwrap().chars().count(),
        30
    );
    let first_text = full["results"][0]["text"].as_str().unwrap();
    let one_row = call(
        client,
        "search",
        json!({ "question": QUESTION, "max_chars": first_text.chars().count() + 1 }),
    )
    .await;
    let one_row = structured(&one_row);
    assert_eq!(
        one_row["results"],
        json!([full["results"][0]]),
        "a later row that does not fit waits for the next page whole"
    );
    assert_eq!(one_row["truncated"], true);
    assert!(one_row["cursor"].is_string());
}

async fn a_search_inside_a_section_finds_only_its_items(client: &Client) {
    let sample = sample_document_id().to_string();
    let outline = call(client, "document_outline", json!({ "document_id": sample })).await;
    let sections = structured(&outline)["sections"].as_array().unwrap().len();
    let mut found_in_a_section = false;
    for index in 1..=sections {
        let read = call(
            client,
            "read_section",
            json!({ "document_id": sample, "section": index }),
        )
        .await;
        let held = structured(&read)["item_ids"].as_array().unwrap().clone();
        let scoped = call(
            client,
            "search",
            json!({ "question": QUESTION, "document": sample, "section": index }),
        )
        .await;
        let scoped = structured(&scoped);
        for row in scoped["results"].as_array().unwrap() {
            assert!(
                held.contains(&row["item_id"]),
                "section {index} holds {row}"
            );
            assert_eq!(row["section"]["index"], index);
            found_in_a_section = true;
        }
    }
    assert!(found_in_a_section);
    let without_document = call(
        client,
        "search",
        json!({ "question": QUESTION, "section": 1 }),
    )
    .await;
    assert!(error_text(&without_document).contains("`document_id`"));
}
