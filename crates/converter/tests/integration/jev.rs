//! Calls the real Jev API on sample pages, because nothing offline can tell whether its answers
//! still pick out the math, or how it replies to a bad key.

use std::path::PathBuf;

use converter::jev::{Jev, JevError, MathPlacement};

const RUN_COMMAND: &str = "set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p converter --test integration -- --ignored jev::";

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
#[ignore = "calls the real Jev API and spends API credit; run with: set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p converter --test integration -- --ignored jev::"]
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
#[ignore = "calls the real Jev API and spends API credit; run with: set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p converter --test integration -- --ignored jev::"]
async fn bad_key_is_rejected() {
    require_prod_api();
    let jev = Jev::new("not-a-real-key").expect("client builds");

    let error = jev
        .contains_math("x_1 + x_2 = y")
        .await
        .expect_err("a made-up key must not be accepted");

    assert!(
        matches!(error, JevError::Rejected { status: 401, .. }),
        "expected Rejected with status 401, got {error:?}"
    );
}
