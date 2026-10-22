//! Calls the real Gemini embeddings API, because only the real API can show that the request
//! shapes and the picture-with-text request are still accepted.

use std::path::{Path, PathBuf};

use rag_core::{Config, DocumentInput, EMBEDDING_DIMENSIONS, Embedder, GeminiEmbedder};

const RUN_COMMAND: &str = "set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-core --test integration -- --ignored gemini_live::";

fn require_prod_api() {
    assert_eq!(
        std::env::var("REX_PROD_API").as_deref(),
        Ok("true"),
        "this test calls the real Gemini API; run it with: {RUN_COMMAND}"
    );
}

fn largest_sample_page() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/content/option-volatility-and-pricing/chapter-1/page-num-7")
}

fn assert_unit_vector(vector: &[f32], what: &str) {
    assert_eq!(vector.len(), EMBEDDING_DIMENSIONS, "{what}: length");
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 0.01, "{what}: norm is {norm}");
}

#[tokio::test]
#[ignore = "calls the real Gemini API and spends API credit; run with: set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-core --test integration -- --ignored gemini_live::"]
async fn embeds_two_texts_a_figure_and_a_query_live() {
    require_prod_api();
    let config = Config::load().expect("the settings should load");
    let embedder = GeminiEmbedder::from_config(&config)
        .expect("EMBEDDING_GEMINI_API_KEY must be set; source .env first");

    let page = largest_sample_page();
    let explanation = std::fs::read_to_string(page.join("04-figure.md")).unwrap();
    let inputs = [
        DocumentInput {
            title: "Options, chapter 1: Sample Pages > Delta".to_owned(),
            text: "The delta measures how much an option's price moves when the underlying moves."
                .to_owned(),
            image: None,
        },
        DocumentInput {
            title: String::new(),
            text: "A forward contract fixes a price today for a purchase later.".to_owned(),
            image: None,
        },
        DocumentInput {
            title: "Options, chapter 1: Sample Pages > Volatility".to_owned(),
            text: explanation,
            image: Some(page.join("04-figure.png")),
        },
    ];

    let documents = embedder.embed_document(&inputs).await.unwrap();
    assert_eq!(
        documents.len(),
        3,
        "one vector for each input, the figure included"
    );
    for (vector, what) in documents.iter().zip(["delta", "forward", "figure"]) {
        assert_unit_vector(vector, what);
    }

    let query = embedder.embed_query("what is delta").await.unwrap();
    assert_unit_vector(&query, "query");
}
