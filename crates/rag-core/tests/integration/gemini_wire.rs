//! Checks how `GeminiEmbedder` treats a busy and a refusing API, against a local stand-in for it.

use std::time::Duration;

use rag_core::{ApiKey, DocumentInput, EmbedError, Embedder, GeminiEmbedder};

use crate::gemini_stub::GeminiStub;

const FIRST_RETRY_DELAY: Duration = Duration::from_millis(10);

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
