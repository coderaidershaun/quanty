//! Checks that an ingest does not go on when it cannot: a signed-out Claude blocks the check of a
//! new PDF, and stores that are down fail the check and the start with nothing paid for.

use std::sync::Arc;

use backend::live::LiveContext;
use contract::{ChapterState, FailureKind};
use ocr::testing::{Scenario, StubServices};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores};

use super::{assert_consumed, checked, request, started};
use crate::support;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p backend --test integration -- --ignored ingest::"]
async fn a_signed_out_claude_blocks_the_check_of_a_new_pdf() {
    let stores = ThrowawayStores::new("ingest-signed-out");
    let config = stores.config().clone();
    let home = stores.temporary_folder().to_path_buf();
    let pages = Arc::new(StubServices::new(Scenario::SampleChapter));
    let services = support::StandInServices {
        stores,
        embedder: Box::new(StandInEmbedder::default),
        llm: Box::new(|_work| StandInLlm::signed_out()),
        pages: Arc::clone(&pages),
    };
    let cx = LiveContext::new(config, services).with_home(home);

    let preflight = checked(&cx, &request(&cx))
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
    let wanted = request(&cx);
    let stores = [FailureKind::FalkorDbDown, FailureKind::QdrantDown];

    let failure = checked(&cx, &wanted)
        .await
        .expect_err("a check needs a store");

    assert!(stores.contains(&failure.kind), "{failure:?}");
    let (progress, finished) = started(&cx, &wanted).await;
    assert!(progress.is_empty(), "nothing was paid for: {progress:?}");
    let failure = finished.expect_err("a start needs the stores");
    assert!(stores.contains(&failure.kind), "{failure:?}");
    assert_consumed(&cx, &wanted, &ocr::testing::sample_pdf());
    let again = checked(&cx, &wanted)
        .await
        .expect_err("the same receipt can still check its PDF");
    assert!(stores.contains(&again.kind), "{again:?}");
    let (progress, result) = started(&cx, &wanted).await;
    assert!(progress.is_empty());
    let again = result.expect_err("the same receipt retries ingestion up to the store failure");
    assert!(stores.contains(&again.kind), "{again:?}");
    assert_consumed(&cx, &wanted, &ocr::testing::sample_pdf());
    for file_name in ["../outside.pdf", "folder\\outside.pdf"] {
        let invalid = contract::IngestRequest {
            file_name: file_name.into(),
            ..wanted.clone()
        };
        assert_eq!(
            checked(&cx, &invalid).await.unwrap_err().kind,
            FailureKind::BadFile
        );
    }
    assert_eq!(pages.calls(), [], "no page was converted");
}
