//! Checks how `GeminiEmbedder` treats a busy and a refusing API, against a local stand-in for it.

use std::time::Duration;

use rag_core::{ApiKey, DocumentInput, EMBEDDING_MODEL, EmbedError, Embedder, GeminiEmbedder};

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

#[tokio::test]
async fn a_key_check_costs_nothing_and_tells_a_refused_key_from_spent_credit() {
    let accepting = GeminiStub::start(vec![]).await;
    embedder_for(&accepting).check_key().await.unwrap();
    let requests = accepting.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET", "a model read is not billed");
    assert_eq!(
        requests[0].target,
        format!("/models/{EMBEDDING_MODEL}"),
        "the key must not be in the address"
    );
    assert_eq!(requests[0].api_key.as_deref(), Some("not-a-real-key"));

    // Status, message, whether the key is refused, whether the credit is spent.
    let answers = [
        (
            400,
            "API key not valid. Please pass a valid API key.",
            true,
            false,
        ),
        (401, "stub", true, false),
        (403, "stub", true, false),
        (402, "Your prepayment credits are depleted.", false, true),
        (
            429,
            "Your prepay credits are depleted. Add credit.",
            false,
            true,
        ),
        (429, "Resource has been exhausted.", false, false),
        (503, "stub", false, false),
    ];
    for (status, message, is_key_refused, is_out_of_credit) in answers {
        let stub = GeminiStub::start_answering(vec![(status, message)]).await;
        let error = embedder_for(&stub).check_key().await.unwrap_err();
        assert!(
            matches!(error, EmbedError::Rejected { status: got, .. } if got == status),
            "{error:?}"
        );
        assert_eq!(error.is_key_refused(), is_key_refused, "{status} {message}");
        assert_eq!(
            error.is_out_of_credit(),
            is_out_of_credit,
            "{status} {message}"
        );
        assert_eq!(
            stub.requests().len(),
            1,
            "a check is not repeated, even on {status}"
        );
    }
}
