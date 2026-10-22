//! Checks that an ingest does not go on when it cannot: a signed-out Claude blocks the check of a
//! new PDF, and stores that are down fail the check and the start with nothing paid for.

use std::sync::Arc;

use gui::backend::live::LiveContext;
use gui::contract::{ChapterState, FailureKind};
use ocr::testing::{Scenario, StubServices};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores};

use super::{checked, request, started};
use crate::support;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored ingest::"]
async fn a_signed_out_claude_blocks_the_check_of_a_new_pdf() {
    let stores = ThrowawayStores::new("ingest-signed-out");
    let config = stores.config().clone();
    let pages = Arc::new(StubServices::new(Scenario::SampleChapter));
    let services = support::StandInServices {
        stores,
        embedder: Box::new(StandInEmbedder::default),
        llm: Box::new(|_work| StandInLlm::signed_out()),
        pages: Arc::clone(&pages),
    };
    let cx = LiveContext::new(config, services);

    let preflight = checked(&cx, &request())
        .await
        .expect("a check of a readable pdf over stores that answer");

    assert_eq!(
        (preflight.state, preflight.pages),
        (Some(ChapterState::New), Some(7))
    );
    let kinds: Vec<FailureKind> = preflight
        .blockers
        .iter()
        .map(|blocker| blocker.kind)
        .collect();
    assert_eq!(kinds, [FailureKind::ClaudeSignedOut]);
    assert_eq!(pages.calls(), [], "no page was converted");
}

#[tokio::test]
async fn with_the_stores_down_the_check_and_the_start_fail_with_the_store_named_and_nothing_is_paid()
 {
    let folder = tempfile::tempdir().expect("a temporary folder should be made");
    let (cx, pages) = support::closed_with_stub_pages(&folder.path().join("content"));
    let wanted = request();
    let stores = [FailureKind::FalkorDbDown, FailureKind::QdrantDown];

    let failure = checked(&cx, &wanted)
        .await
        .expect_err("a check needs a store");

    assert!(stores.contains(&failure.kind), "{failure:?}");
    let (progress, finished) = started(&cx, &wanted).await;
    assert!(progress.is_empty(), "nothing was paid for: {progress:?}");
    let failure = finished.expect_err("a start needs the stores");
    assert!(stores.contains(&failure.kind), "{failure:?}");
    assert_eq!(pages.calls(), [], "no page was converted");
}
