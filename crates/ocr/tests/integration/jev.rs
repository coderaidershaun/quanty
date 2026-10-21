//! The live tests call the real Jev API on sample pages, because nothing offline can tell whether
//! its answers still pick out the math, or how it replies to a bad key. The key check is tried
//! against a stand-in on a local port, so it never reaches the real API.

use std::path::PathBuf;

use ocr::convert::services::{Jev, JevError, MathPlacement};

use crate::jev_stub::JevStub;

const RUN_COMMAND: &str = "set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p ocr --test integration -- --ignored jev::";

const ANY_MATH: &[Option<MathPlacement>] = &[
    Some(MathPlacement::Inline),
    Some(MathPlacement::Block),
    Some(MathPlacement::Both),
];

fn require_prod_api() {
    assert_eq!(
        std::env::var("REX_PROD_API").as_deref(),
        Ok("true"),
        "this test calls the real Jev API; run it with: {RUN_COMMAND}"
    );
}

fn fixture(file_name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/integration/fixtures/jev")
        .join(file_name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()))
}

#[tokio::test]
#[ignore = "calls the real Jev API and spends API credit; run with: set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p ocr --test integration -- --ignored jev::"]
async fn contains_math_matches_sample_pages() {
    require_prod_api();
    let jev = Jev::from_env().expect("CONVERTER_JEV_API_KEY must be set; source .env first");

    // The only formula on the word-ratio page is a ratio of spelled-out Greek letter names. It
    // must be reported as math, but where it sits is not pinned down.
    let cases: [(&str, &[Option<MathPlacement>]); 5] = [
        ("inline-and-block.txt", &[Some(MathPlacement::Both)]),
        ("block-only.txt", &[Some(MathPlacement::Block)]),
        ("inline-only.txt", &[Some(MathPlacement::Inline)]),
        ("word-ratio-and-table.txt", ANY_MATH),
        ("word-formula-and-arithmetic.txt", &[None]),
    ];

    let mut mismatches = Vec::new();
    for (file_name, accepted) in cases {
        let found = jev
            .contains_math(&fixture(file_name))
            .await
            .unwrap_or_else(|error| panic!("{file_name}: request failed: {error:?}"));
        if !accepted.contains(&found) {
            mismatches.push(format!(
                "{file_name}: expected one of {accepted:?}, got {found:?}"
            ));
        }
    }

    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[tokio::test]
async fn a_key_check_costs_nothing_and_tells_a_refused_key_from_spent_credit() {
    let jev_for = |stub: &JevStub| {
        Jev::new("not-a-real-key")
            .unwrap()
            .with_base_url(&stub.base_url())
    };

    let accepting = JevStub::start(vec![]).await;
    jev_for(&accepting).check_key().await.unwrap();
    let requests = accepting.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].method, "GET",
        "listing the models uses no tokens"
    );
    assert_eq!(requests[0].target, "/v1/models");
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Bearer not-a-real-key")
    );

    // Status, whether the key is refused, whether the credit is spent.
    let answers = [
        (401, true, false),
        (403, true, false),
        (402, false, true),
        (503, false, false),
    ];
    for (status, is_key_refused, is_out_of_credit) in answers {
        let stub = JevStub::start(vec![status]).await;
        let error = jev_for(&stub).check_key().await.unwrap_err();
        assert!(
            matches!(error, JevError::Rejected { status: got, .. } if got == status),
            "{error:?}"
        );
        assert_eq!(error.is_key_refused(), is_key_refused, "{status}");
        assert_eq!(error.is_out_of_credit(), is_out_of_credit, "{status}");
        assert_eq!(
            stub.requests().len(),
            1,
            "a check is not repeated, even on {status}"
        );
    }
}
