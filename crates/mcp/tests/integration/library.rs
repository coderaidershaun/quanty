//! Checks what an agent can read of the library: the documents that are stored, and the pieces of
//! one page of a converted chapter.

use serde_json::json;

use crate::support::{
    AUTHOR, ClosedPorts, Library, call, connect, sample_chapter_folder, sample_content_folder,
    sample_document_id, structured,
};

#[tokio::test(flavor = "multi_thread")]
async fn read_page_gives_the_pieces_of_a_page_in_reading_order() {
    let ports = ClosedPorts::new().with_content_folder(&sample_content_folder());
    let client = connect(ports.server()).await;
    let document_id = sample_document_id().to_string();

    let result = call(
        &client,
        "read_page",
        json!({ "document_id": document_id, "page": 4 }),
    )
    .await;

    let page = structured(&result);
    assert_eq!(page["document_id"], document_id.as_str());
    assert_eq!(page["book"], "Option Volatility and Pricing");
    assert_eq!(page["chapter_number"], 1);
    assert_eq!(page["chapter_name"], "Sample Pages");
    assert_eq!(page["page"], 4);
    assert_eq!(page["pages"], 7);
    assert_eq!(page["printed_page"], "147");
    assert_eq!(
        page["page_image"],
        sample_chapter_folder()
            .join("page-num-4/page.png")
            .display()
            .to_string()
    );
    let pieces = page["pieces"].as_array().unwrap();
    let kinds: Vec<&str> = pieces
        .iter()
        .map(|piece| piece["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "text", "formula", "text", "formula", "text", "heading", "text", "text", "text"
        ]
    );
    let numbers: Vec<u64> = pieces
        .iter()
        .map(|piece| piece["number"].as_u64().unwrap())
        .collect();
    assert_eq!(numbers, (1..=9).collect::<Vec<u64>>());
    let formula = &pieces[1];
    assert_eq!(formula["label"], "(7.3)");
    assert!(
        formula["content"]
            .as_str()
            .unwrap()
            .starts_with("\\begin{aligned} \\ell(\\theta, u)")
    );
    assert_eq!(
        formula["section"],
        json!(["Forward Pricing", "The Delta", "Rate of Change"])
    );
    // The heading on this page closes the two inner sections and opens its own.
    assert_eq!(
        pieces[6]["section"],
        json!([
            "Forward Pricing",
            "Sequential and Maximum Likelihood Estimation in Simplified Regular Vine Copulas"
        ])
    );

    let result = call(
        &client,
        "read_page",
        json!({ "document_id": document_id, "page": 5 }),
    )
    .await;

    let page = structured(&result);
    let figure = &page["pieces"][0];
    assert_eq!(figure["kind"], "figure");
    assert_eq!(figure["label"], "Figure 13-4");
    let picture = sample_chapter_folder().join("page-num-5/01-figure.png");
    assert_eq!(figure["picture"], picture.display().to_string());
    assert!(picture.is_file());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored library::"]
async fn list_documents_shows_each_stored_document_with_its_labels_and_chapter() {
    let library = Library::ingested("mcp-documents").await;
    let client = connect(library.server()).await;

    let result = call(&client, "list_documents", json!({})).await;

    let documents = structured(&result)["documents"].as_array().unwrap();
    let titles: Vec<&str> = documents
        .iter()
        .map(|document| document["title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        [
            "Option Volatility and Pricing, chapter 1: Sample Pages",
            "Quanty Sample Notes, chapter 1: Options Pricing Intuition",
            "Quanty Sample Notes, chapter 2: Black Scholes In Depth",
        ]
    );
    assert_eq!(
        documents[0],
        json!({
            "document_id": sample_document_id().to_string(),
            "title": "Option Volatility and Pricing, chapter 1: Sample Pages",
            "book": "Option Volatility and Pricing",
            "author": AUTHOR,
            "tags": ["options"],
            "chapter_number": 1,
            "chapter_name": "Sample Pages",
            "pages": 7,
        })
    );
    assert_eq!(documents[1]["book"], "Quanty Sample Notes");
    assert!(documents[1].get("author").is_none());
    assert_eq!(documents[1]["tags"], json!([]));
    assert_eq!(documents[1]["chapter_number"], 1);
    assert_eq!(documents[1]["pages"], 3);
    assert_eq!(documents[2]["chapter_number"], 2);
    assert_eq!(documents[2]["chapter_name"], "Black Scholes In Depth");
}
