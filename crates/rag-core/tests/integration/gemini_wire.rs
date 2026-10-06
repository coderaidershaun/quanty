//! Checks what `GeminiEmbedder` puts on the wire and how it treats the answers, against a local
//! stand-in for the API.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use rag_core::{ApiKey, DocumentInput, EMBEDDING_DIMENSIONS, EmbedError, Embedder, GeminiEmbedder};

use crate::gemini_stub::GeminiStub;

const FIRST_RETRY_DELAY: Duration = Duration::from_millis(10);

fn smallest_sample_figure() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../samples/content/option-volatility-and-pricing/chapter-1/page-num-5/01-figure.png",
    )
}

fn embedder_for(stub: &GeminiStub) -> GeminiEmbedder {
    GeminiEmbedder::new(&ApiKey::new("not-a-real-key"))
        .unwrap()
        .with_endpoint(&stub.base_url(), FIRST_RETRY_DELAY)
}

fn text_input(title: &str, text: &str) -> DocumentInput {
    DocumentInput {
        title: title.to_owned(),
        text: text.to_owned(),
        image: None,
    }
}

fn figure_input(text: &str, picture: PathBuf) -> DocumentInput {
    DocumentInput {
        title: "context".to_owned(),
        text: text.to_owned(),
        image: Some(picture),
    }
}

/// 101 texts numbered from 0, the first with no title, and a figure numbered 900 at place 50.
/// The stand-in server answers each input with its number, so the numbers in input order are
/// what the returned vectors must carry.
fn texts_around_a_figure(picture: &Path) -> (Vec<DocumentInput>, Vec<f32>) {
    let mut inputs: Vec<DocumentInput> = (0..101)
        .map(|number| {
            text_input(
                if number == 0 { "" } else { "context" },
                &number.to_string(),
            )
        })
        .collect();
    inputs.insert(
        50,
        figure_input("the figure explained 900", picture.to_path_buf()),
    );
    let mut numbers: Vec<f32> = (0..101).map(|number| number as f32).collect();
    numbers.insert(50, 900.0);
    (inputs, numbers)
}

#[tokio::test]
async fn texts_go_in_batches_and_a_figure_goes_alone_with_its_picture() {
    let stub = GeminiStub::start(vec![]).await;
    let embedder = embedder_for(&stub);
    let picture = smallest_sample_figure();

    let (inputs, expected_tags) = texts_around_a_figure(&picture);

    let vectors = embedder.embed_document(&inputs).await.unwrap();
    let first_values: Vec<f32> = vectors.iter().map(|vector| vector[0]).collect();
    assert_eq!(
        first_values, expected_tags,
        "each vector belongs to its input"
    );
    assert!(
        vectors
            .iter()
            .all(|vector| vector.len() == EMBEDDING_DIMENSIONS)
    );

    let query = embedder.embed_query("7").await.unwrap();
    assert_eq!(query[0], 7.0);

    let calls = stub.requests();
    let paths: Vec<&str> = calls.iter().map(|call| call.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/models/gemini-embedding-2:batchEmbedContents",
            "/models/gemini-embedding-2:batchEmbedContents",
            "/models/gemini-embedding-2:embedContent",
            "/models/gemini-embedding-2:embedContent",
        ]
    );
    assert!(
        calls
            .iter()
            .all(|call| call.api_key.as_deref() == Some("not-a-real-key"))
    );

    let first_batch = calls[0].body["requests"].as_array().unwrap();
    let second_batch = calls[1].body["requests"].as_array().unwrap();
    assert_eq!((first_batch.len(), second_batch.len()), (100, 1));
    for item in first_batch.iter().chain(second_batch) {
        assert_eq!(item["model"], "models/gemini-embedding-2");
        assert_eq!(item["output_dimensionality"], 768);
    }
    assert_eq!(
        first_batch[0]["content"]["parts"][0]["text"],
        "title: none | text: 0"
    );
    assert_eq!(
        first_batch[1]["content"]["parts"][0]["text"],
        "title: context | text: 1"
    );

    let figure_body = &calls[2].body;
    assert_eq!(figure_body["output_dimensionality"], 768);
    let parts = figure_body["content"]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(
        parts[0]["text"],
        "title: context | text: the figure explained 900"
    );
    assert_eq!(parts[1]["inline_data"]["mime_type"], "image/png");
    let sent_picture = base64::engine::general_purpose::STANDARD
        .decode(parts[1]["inline_data"]["data"].as_str().unwrap())
        .unwrap();
    assert_eq!(sent_picture, std::fs::read(&picture).unwrap());

    assert_eq!(
        calls[3].body["content"]["parts"][0]["text"],
        "task: search result | query: 7"
    );

    let lost_picture = figure_input(
        "a figure whose picture is gone",
        picture.with_file_name("no-such-figure.png"),
    );
    let error = embedder
        .embed_document(&[text_input("context", "1"), lost_picture])
        .await
        .unwrap_err();
    assert!(matches!(error, EmbedError::Image { .. }), "{error:?}");
    assert_eq!(
        stub.requests().len(),
        calls.len(),
        "a picture that cannot be read stops the call before any text is sent and paid for"
    );
}

#[tokio::test]
async fn a_rate_limited_call_is_tried_again_and_a_refused_one_is_not() {
    let busy = GeminiStub::start(vec![429, 503]).await;
    let vectors = embedder_for(&busy)
        .embed_document(&[text_input("context", "3")])
        .await
        .unwrap();
    assert_eq!(vectors[0][0], 3.0);
    assert_eq!(
        busy.requests().len(),
        3,
        "two failures, then the good reply"
    );

    let refusing = GeminiStub::start(vec![400]).await;
    let error = embedder_for(&refusing)
        .embed_document(&[text_input("context", "3")])
        .await
        .unwrap_err();
    assert!(
        matches!(error, EmbedError::Rejected { status: 400, .. }),
        "{error:?}"
    );
    assert_eq!(
        refusing.requests().len(),
        1,
        "a bad request is not repeated"
    );
}
