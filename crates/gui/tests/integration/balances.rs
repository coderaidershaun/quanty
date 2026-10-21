//! Checks what the live backend makes of each answer to its key checks, that spent credit fails an
//! ask or an ingest in words of its own, and that no key reaches the screen. No provider is called:
//! the checks are stand-ins, or run against a port that nothing listens on.

use std::sync::Arc;
use std::time::Duration;

use egui_kittest::kittest::{By, NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::live::LiveContext;
use gui::backend::{Handler, Reply};
use gui::contract::{
    Balance, BalanceReads, Command, Event, Failure, FailureKind, ReadFailure, RequestId,
    StartupFacts,
};
use gui::testkit;
use ocr::ConvertError;
use ocr::convert::services::{JevError, ServiceError};
use ocr::testing::{Scenario, StubServices};
use rag_core::{ApiKey, Config, EmbedError};

use crate::support::{Check, KeyChecks, StandInsOnClosedPorts, closed_ports_config};

const GEMINI_KEY: &str = "gemini-key-that-must-never-show";
const JEV_KEY: &str = "jev-key-that-must-never-show";
const REQUEST: RequestId = RequestId(7);
const DEPLETED: &str = "Your prepayment credits are depleted. Add credit in AI Studio.";
const INVALID: &str = r#"{"error":{"code":400,"message":"API key not valid.","details":[{"reason":"API_KEY_INVALID"}]}}"#;

fn rejected(status: u16, body: &'static str) -> Check {
    Check::Rejected { status, body }
}

#[derive(Clone, Copy, PartialEq)]
enum Keys {
    InTheSettings,
    Missing,
}

fn context(gemini: Check, jev: Check, keys: Keys) -> LiveContext<KeyChecks<StandInsOnClosedPorts>> {
    let key = |text| (keys == Keys::InTheSettings).then(|| ApiKey::new(text));
    let config = Config {
        gemini_api_key: key(GEMINI_KEY),
        jev_api_key: key(JEV_KEY),
        ..closed_ports_config()
    };
    let inner = StandInsOnClosedPorts {
        pages: Arc::new(StubServices::new(Scenario::SampleChapter)),
    };
    LiveContext::new(config, KeyChecks { inner, gemini, jev })
}

async fn found_by(cx: &LiveContext<KeyChecks<StandInsOnClosedPorts>>) -> BalanceReads {
    let (reply, events, _stop) = Reply::collecting();
    cx.serve(Command::ReadBalances { request: REQUEST }, reply)
        .await;
    let sent: Vec<Event> = events.try_iter().collect();
    match sent.as_slice() {
        [Event::Balances { request, found }] if *request == REQUEST => *found,
        other => panic!("expected one answer to the read: {other:#?}"),
    }
}

fn reads(google: Balance, jev: Balance) -> BalanceReads {
    BalanceReads { google, jev }
}

#[test]
fn the_live_backend_names_what_each_provider_answered_and_never_shows_a_key() {
    every_answer_of_a_provider_has_its_words();
    spent_credit_fails_a_job_in_words_of_its_own();
    a_key_is_never_shown_on_the_screen();
}

fn every_answer_of_a_provider_has_its_words() {
    let unreachable = Check::Unreachable;
    let cases = [
        // With no key in the settings no check is called, or this one would find a status 500.
        (
            context(rejected(500, ""), rejected(500, ""), Keys::Missing),
            reads(Balance::KeyMissing, Balance::KeyMissing),
        ),
        (
            context(Check::Accepted, Check::Accepted, Keys::InTheSettings),
            reads(Balance::NotOffered, Balance::NotOffered),
        ),
        (
            context(
                rejected(402, DEPLETED),
                rejected(401, ""),
                Keys::InTheSettings,
            ),
            reads(Balance::OutOfCredit, Balance::KeyRefused),
        ),
        (
            context(
                rejected(400, INVALID),
                rejected(402, ""),
                Keys::InTheSettings,
            ),
            reads(Balance::KeyRefused, Balance::OutOfCredit),
        ),
        (
            context(rejected(503, ""), unreachable, Keys::InTheSettings),
            reads(
                Balance::Unread(ReadFailure::Status(503)),
                Balance::Unread(ReadFailure::Unreachable),
            ),
        ),
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime should start");
    for (cx, wanted) in &cases {
        assert_eq!(runtime.block_on(found_by(cx)), *wanted);
    }
}

fn spent_credit_fails_a_job_in_words_of_its_own() {
    let google = EmbedError::Rejected {
        status: 402,
        body: DEPLETED.to_owned(),
    };
    assert_eq!(
        Failure::from(google).kind,
        FailureKind::EmbeddingOutOfCredit
    );
    let jev = ConvertError::Services(ServiceError::Jev(JevError::Rejected {
        status: 402,
        body: String::new(),
    }));
    assert_eq!(Failure::from(jev).kind, FailureKind::ConverterOutOfCredit);
}

/// Every node of the window, with everything its accessibility data says, written out.
fn everything_the_nodes_say(harness: &egui_kittest::Harness<'_, gui::app::App>) -> String {
    harness
        .query_all(By::new().predicate(|_| true))
        .map(|node| format!("{:?}", node.accesskit_node().data()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn a_key_is_never_shown_on_the_screen() {
    let cx = context(
        rejected(402, DEPLETED),
        rejected(401, ""),
        Keys::InTheSettings,
    );
    let facts = StartupFacts {
        home: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        env_file: None,
        fixture: None,
        anthropic_api_key_set: false,
    };
    let mut harness = testkit::app_on(cx, facts, Vec::new(), DEFAULT_WINDOW);
    testkit::settle_within(&mut harness, Duration::from_secs(10));

    let said = everything_the_nodes_say(&harness);
    assert!(said.contains("Google Gemini: out of credit"), "{said}");
    assert!(said.contains("Jev: key refused"), "{said}");
    for key in [GEMINI_KEY, JEV_KEY] {
        assert!(!said.contains(key), "a node says the key {key}");
    }
}
