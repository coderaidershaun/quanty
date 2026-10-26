//! Checks what an agent can read of the library: the pieces and items of a converted chapter, the
//! documents that are stored with their concepts, and the words of a spent credit.

use rag_core::{DocId, EmbedError};
use rag_ingestion::chapter_items;
use serde_json::json;

use crate::support::{
    AUTHOR, Client, ClosedPorts, Library, StandInServices, call, connect, error_text,
    sample_chapter_folder, sample_content_folder, sample_document_id, sample_notes_document_id,
    structured,
};

#[tokio::test(flavor = "multi_thread")]
async fn read_page_and_chapter_contents_give_a_chapter_as_it_was_converted() {
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
    assert_eq!(page["media"], "Option Volatility and Pricing");
    assert!(
        page.get("document_title").is_none(),
        "a chapter has no title of its own"
    );
    assert_eq!(page["chapter_number"], 1);
    assert_eq!(page["chapter_name"], "Sample Pages");
    assert_eq!(page["page"], 4);
    assert_eq!(page["pages"], 7);
    assert_eq!(page["printed_page"], "147");
    assert_eq!(
        page["page_image"],
        "content/option-volatility-and-pricing/chapter-1/page-num-4/page.png"
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
    assert_eq!(formula["name"], "Likelihood of the D-vine copula");
    assert!(
        formula.get("caption").is_none() && pieces[0].get("name").is_none(),
        "only a formula has a name, and only a figure, table or listing a caption"
    );
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
    assert_eq!(
        figure["picture"],
        "content/option-volatility-and-pricing/chapter-1/page-num-5/01-figure.png"
    );
    assert!(picture.is_file());
    assert!(
        figure.get("caption").is_none(),
        "a figure with no caption has no caption key"
    );

    let result = call(
        &client,
        "read_page",
        json!({ "document_id": document_id, "page": 6 }),
    )
    .await;

    let captioned = &structured(&result)["pieces"][0];
    assert_eq!(captioned["label"], "Figure 13-16");
    assert_eq!(captioned["caption"], "Dividend sensitivity.");

    let chapter =
        ocr::ChapterIndex::read(&sample_content_folder().join("quanty-sample-notes/chapter-2"))
            .expect("the notes chapter should have a chapter.json");
    let document_id = DocId::from_source_sha256(&chapter.source_sha256).to_string();

    let result = call(
        &client,
        "read_page",
        json!({ "document_id": document_id, "page": 3 }),
    )
    .await;

    let pieces = structured(&result)["pieces"].as_array().unwrap();
    assert_eq!(pieces.len(), 11);
    let listing = &pieces[10];
    assert_eq!(listing["number"], 11);
    assert_eq!(listing["kind"], "code");
    assert_eq!(listing["label"], "Listing 2.1");
    assert_eq!(listing["language"], "python");
    assert_eq!(
        listing["content"],
        "def call_price(spot, strike, rate, volatility, years):\n    d1 = (log(spot / strike) + (rate + 0.5 * volatility ** 2) * years) / (volatility * sqrt(years))\n    d2 = d1 - volatility * sqrt(years)\n\n    return spot * norm_cdf(d1) - strike * exp(-rate * years) * norm_cdf(d2)"
    );
    assert!(
        pieces[9].get("language").is_none(),
        "a piece that is not code has no language"
    );

    let notes = sample_notes_document_id(2).to_string();

    let result = call(&client, "chapter_contents", json!({ "document_id": notes })).await;

    let contents = structured(&result);
    assert_eq!(contents["document_id"], notes.as_str());
    assert_eq!(contents["media"], "Quanty Sample Notes");
    assert_eq!(contents["chapter_number"], 2);
    assert_eq!(contents["chapter_name"], "Black Scholes In Depth");
    assert_eq!(contents["pages"], 3);
    let counts = json!({ "formula": 6, "figure": 0, "table": 0, "code": 1 });
    assert_eq!(contents["counts"], counts);
    let items = contents["items"].as_array().unwrap();
    let labels: Vec<&str> = items
        .iter()
        .map(|item| item["label"].as_str().unwrap())
        .collect();
    assert_eq!(
        labels,
        [
            "(2.1)",
            "(2.2)",
            "(2.3)",
            "(2.4)",
            "(2.5)",
            "(2.6)",
            "Listing 2.1"
        ]
    );
    let pages: Vec<u64> = items
        .iter()
        .map(|item| item["page"].as_u64().unwrap())
        .collect();
    assert_eq!(pages, [1, 2, 3, 3, 3, 3, 3]);
    assert_eq!(items[3]["kind"], "formula");
    assert_eq!(items[3]["name"], "Black–Scholes call price");
    let listing = &items[6];
    assert_eq!(listing["kind"], "code");
    assert_eq!(listing["caption"], "The call price in Python");
    assert_eq!(listing["language"], "python");
    assert!(
        listing.get("last_page").is_none(),
        "a listing on one page has no last page"
    );
    let page = call(
        &client,
        "read_page",
        json!({ "document_id": notes, "page": 3 }),
    )
    .await;
    assert_eq!(
        listing["content"],
        structured(&page)["pieces"][10]["content"]
    );

    let result = call(
        &client,
        "chapter_contents",
        json!({ "document_id": notes, "kind": "code" }),
    )
    .await;

    let code = structured(&result);
    assert_eq!(code["items"], json!([listing]));
    assert_eq!(code["counts"], counts, "the counts are the chapter's");

    let result = call(
        &client,
        "chapter_contents",
        json!({ "document_id": sample_document_id().to_string(), "kind": "figure" }),
    )
    .await;

    let figures = structured(&result)["items"].as_array().unwrap();
    let placed: Vec<(&str, u64)> = figures
        .iter()
        .map(|figure| {
            (
                figure["label"].as_str().unwrap(),
                figure["page"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        placed,
        [("Figure 13-4", 5), ("Figure 13-16", 6), ("Figure 24-12", 7)]
    );
    for figure in figures {
        let relative = figure["picture"]
            .as_str()
            .unwrap()
            .strip_prefix("content/")
            .unwrap();
        let picture = sample_content_folder().join(relative);
        assert!(picture.is_file(), "{picture:?}");
    }

    let chunk = call(
        &client,
        "chapter_contents",
        json!({ "document_id": notes, "kind": "chunk" }),
    )
    .await;

    let text = error_text(&chunk);
    assert!(text.contains("`read_page`"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored library::"]
async fn the_stored_library_is_listed_its_concepts_are_read_and_a_spent_credit_is_named() {
    let library = Library::ingested_with_concepts("mcp-library").await;
    let client = connect(library.server()).await;

    list_documents_shows_each_stored_document_with_its_labels_and_chapter(&client).await;
    chapter_concepts_gives_the_concepts_of_a_chapter_and_of_a_page(&client).await;
    a_spent_credit_is_named_with_its_billing_page(&library).await;
}

async fn list_documents_shows_each_stored_document_with_its_labels_and_chapter(client: &Client) {
    let result = call(client, "list_documents", json!({})).await;

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
            "media": "Option Volatility and Pricing",
            "category": "book",
            "authors": [AUTHOR],
            "media_tags": ["options"],
            "tags": [],
            "chapter_number": 1,
            "chapter_name": "Sample Pages",
            "pages": 7,
            "ingested_whole": true,
            "items": { "chunk": 9, "formula": 5, "figure": 3, "table": 3, "code": 0 },
        })
    );
    assert_eq!(documents[1]["media"], "Quanty Sample Notes");
    assert_eq!(documents[1]["category"], "book");
    assert_eq!(documents[1]["authors"], json!([]));
    assert_eq!(documents[1]["media_tags"], json!([]));
    assert_eq!(documents[1]["tags"], json!([]));
    assert_eq!(documents[1]["chapter_number"], 1);
    assert_eq!(documents[1]["pages"], 3);
    assert_eq!(documents[2]["chapter_number"], 2);
    assert_eq!(documents[2]["chapter_name"], "Black Scholes In Depth");
}

async fn chapter_concepts_gives_the_concepts_of_a_chapter_and_of_a_page(client: &Client) {
    let sample = sample_document_id().to_string();
    let chapter = ocr::read_chapter(&sample_chapter_folder()).expect("the sample chapter reads");
    let straddle_page = chapter_items(&chapter)
        .into_iter()
        .find(|item| item.input.text.contains("straddle"))
        .expect("a sample item mentions a straddle")
        .payload
        .page;
    let long_straddle = json!([{
        "name": "Long straddle",
        "definition": "A bet that the price moves a lot, either way.",
    }]);

    let whole = call(client, "chapter_concepts", json!({ "document_id": sample })).await;

    assert_eq!(
        structured(&whole),
        &json!({
            "document_id": sample,
            "title": "Option Volatility and Pricing, chapter 1: Sample Pages",
            "concepts": long_straddle,
        })
    );

    let on_page = call(
        client,
        "chapter_concepts",
        json!({ "document_id": sample, "page": straddle_page }),
    )
    .await;

    let on_page = structured(&on_page);
    assert_eq!(on_page["page"], straddle_page);
    assert_eq!(on_page["concepts"], long_straddle);
    let other_page = call(
        client,
        "chapter_concepts",
        json!({ "document_id": sample, "page": straddle_page - 1 }),
    )
    .await;
    assert_eq!(structured(&other_page)["concepts"], json!([]));
    let notes = call(
        client,
        "chapter_concepts",
        json!({ "document_id": sample_notes_document_id(1).to_string() }),
    )
    .await;
    assert_eq!(structured(&notes)["concepts"], json!([]));

    let zero = call(
        client,
        "chapter_concepts",
        json!({ "document_id": sample, "page": 0 }),
    )
    .await;
    assert!(error_text(&zero).contains("pages count from 1"));
    let unknown = DocId::from_source_sha256("a pdf that was never stored").to_string();
    let missing = call(
        client,
        "chapter_concepts",
        json!({ "document_id": unknown }),
    )
    .await;
    let text = error_text(&missing);
    assert!(text.contains(&unknown), "{text}");
    assert!(text.contains("`list_documents`"), "{text}");
}

async fn a_spent_credit_is_named_with_its_billing_page(library: &Library) {
    let rejected = |status: u16, body: &'static str| {
        let services = StandInServices::new().with_failing_embedder(move || EmbedError::Rejected {
            status,
            body: body.to_owned(),
        });
        library.server_over(&sample_content_folder(), services)
    };
    let client = connect(rejected(402, "Your prepayment credits are depleted")).await;

    let search = call(&client, "search", json!({ "question": "what is delta?" })).await;

    let text = error_text(&search);
    assert!(
        text.contains(
            " Your Google Gemini prepaid credit is used up: add credit at https://aistudio.google.com/billing, then try again. Call the `health` tool"
        ),
        "{text}"
    );
    assert!(
        text.ends_with("Call the `health` tool to see which service is not ready."),
        "{text}"
    );

    let client = connect(rejected(500, "the service is busy")).await;

    let search = call(&client, "search", json!({ "question": "what is delta?" })).await;

    let text = error_text(&search);
    assert!(text.contains("500"), "{text}");
    assert!(!text.contains("prepaid credit"), "{text}");
}
