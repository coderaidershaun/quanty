//! Checks the report of `health`, which an agent reads before it calls anything else.

use serde_json::{Value, json};

use mcp::QuantyServer;
use rag_ingestion::{KeyReadFailure, KeyState, KeyStates};

use crate::support::{ClosedPorts, StandInServices, call, connect, structured};

#[tokio::test(flavor = "multi_thread")]
async fn health_names_each_service_that_is_not_ready_and_says_why_a_key_cannot_be_used() {
    let ports = ClosedPorts::new();
    let client = connect(ports.server()).await;

    let result = call(&client, "health", json!({})).await;

    let report = structured(&result);
    assert_eq!(report["healthy"], false);
    let lines = report["report"].as_str().unwrap();
    assert!(lines.contains("Qdrant: FAILED"), "{lines}");
    assert!(lines.contains("FalkorDB: FAILED"), "{lines}");
    assert!(lines.contains("agent:"), "{lines}");
    assert!(lines.contains("Gemini key: ok (accepted)"), "{lines}");
    assert!(lines.contains("Jev key: FAILED (missing)"), "{lines}");
    assert_eq!(
        report["keys"],
        json!([
            {
                "key": "EMBEDDING_GEMINI_API_KEY",
                "service": "Google Gemini",
                "state": "accepted",
                "detail": "the key was accepted; Google gives no balance to a key, so the credit is on the billing page",
                "billing_page": "https://aistudio.google.com/billing",
            },
            {
                "key": "CONVERTER_JEV_API_KEY",
                "service": "Jev",
                "state": "missing",
                "detail": "CONVERTER_JEV_API_KEY is not set; add it to .env or to the environment, then start the server again",
                "billing_page": "https://console.typesafe.ai/",
            },
        ])
    );
    let stand_in = json!({ "provider": "stand-in", "model": "stand-in", "thinking": "medium" });
    let model = |work: &str, tools: Value| {
        let mut model = stand_in.clone();
        model["work"] = json!(work);
        model["tools"] = tools;
        model
    };
    assert_eq!(
        report["models"],
        json!([
            model("ask", json!(["search", "answer"])),
            model("ingest", json!(["ingest_pdf", "write_insights"])),
            model("datasets", json!(["build_dataset"])),
        ])
    );

    let asked = async |gemini: KeyState, jev: KeyState| {
        let services = StandInServices::new().with_key_states(KeyStates { gemini, jev });
        let client = connect(QuantyServer::new(ports.config.clone(), services)).await;
        let result = call(&client, "health", json!({})).await;
        structured(&result).clone()
    };

    let refused = asked(
        KeyState::Refused,
        KeyState::Unread(KeyReadFailure::TimedOut),
    )
    .await;

    assert_eq!(refused["healthy"], false);
    assert_eq!(refused["keys"][0]["state"], "refused");
    assert_eq!(
        refused["keys"][0]["detail"],
        "the provider refused the key; check it on the billing page, put a good key in .env, then start the server again"
    );
    assert_eq!(refused["keys"][1]["state"], "unread");
    assert_eq!(
        refused["keys"][1]["detail"],
        "the provider gave no answer within 5 seconds"
    );
    let lines = refused["report"].as_str().unwrap();
    assert!(lines.contains("Gemini key: FAILED (refused)"), "{lines}");
    assert!(lines.contains("Jev key: FAILED (unread)"), "{lines}");

    let spent = asked(
        KeyState::OutOfCredit,
        KeyState::Unread(KeyReadFailure::Status(500)),
    )
    .await;

    assert_eq!(spent["healthy"], false);
    assert_eq!(spent["keys"][0]["state"], "out_of_credit");
    assert_eq!(
        spent["keys"][0]["detail"],
        "the provider says the prepaid credit is used up; add credit on the billing page"
    );
    assert_eq!(
        spent["keys"][1]["detail"],
        "the provider answered with HTTP 500"
    );
    let lines = spent["report"].as_str().unwrap();
    assert!(
        lines.contains("Gemini key: FAILED (out_of_credit)"),
        "{lines}"
    );
    assert_eq!(
        spent["keys"][0]["billing_page"],
        "https://aistudio.google.com/billing"
    );
    assert_eq!(
        spent["keys"][1]["billing_page"],
        "https://console.typesafe.ai/"
    );

    let refused_location = asked(KeyState::LocationRefused, KeyState::Accepted).await;

    assert_eq!(refused_location["healthy"], false);
    assert_eq!(refused_location["keys"][0]["state"], "location_refused");
    let detail = refused_location["keys"][0]["detail"].as_str().unwrap();
    assert!(
        detail.contains("EMBEDDING_GEMINI_IPV4_ONLY=true"),
        "{detail}"
    );
    let lines = refused_location["report"].as_str().unwrap();
    assert!(
        lines.contains("Gemini key: FAILED (location_refused)"),
        "{lines}"
    );
}
