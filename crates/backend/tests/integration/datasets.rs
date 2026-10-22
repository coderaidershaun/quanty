//! Checks that the live backend builds a dataset: a dry run tells every stage over the sample
//! chapters and writes its files, a paid build over a model that is signed out ends before its
//! first step with the model named, and a graph that is down ends the build with the graph named.

use std::path::Path;
use std::sync::Arc;

use backend::live::{LiveContext, Services};
use backend::{Handler, Reply};
use contract::{
    Command, DatasetProgress, DatasetReport, DatasetRequest, DatasetStage, Event, Failure,
    FailureKind, RequestId,
};
use ocr::testing::{Scenario, StubServices};
use rag_core::Config;
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores};

use crate::support;

const REQUEST: RequestId = RequestId(9);
const CHAPTERS: [&str; 3] = [support::SAMPLE_PAGES, support::INTUITION, support::IN_DEPTH];

/// The three sample chapters, in a content folder as the converter lays it out.
fn copy_the_samples_to(content: &Path) {
    for chapter in CHAPTERS {
        support::copy_folder(&support::sample_chapter(chapter), &content.join(chapter));
    }
}

/// What a build told: the progress that came before the end, and the end.
async fn built<S: Services>(
    cx: &LiveContext<S>,
    wanted: &DatasetRequest,
) -> (Vec<DatasetProgress>, Result<DatasetReport, Failure>) {
    let command = Command::BuildDataset {
        request: REQUEST,
        build: wanted.clone(),
    };
    let (reply, events, _stop) = Reply::collecting();
    cx.serve(command, reply).await;
    let events: Vec<Event> = events.try_iter().collect();
    let Some((
        Event::DatasetFinished {
            request: REQUEST,
            result,
        },
        before,
    )) = events.split_last()
    else {
        panic!("the last event must end the build of {REQUEST:?}: {events:#?}");
    };
    let progress = before
        .iter()
        .map(|event| match event {
            Event::DatasetProgress {
                request: REQUEST,
                progress,
            } => progress.clone(),
            other => panic!("only progress comes before the end: {other:#?}"),
        })
        .collect();
    (progress, result.clone())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p backend --test integration -- --ignored datasets::"]
async fn a_dry_run_tells_every_stage_and_a_signed_out_model_stops_a_paid_build() {
    let stores = ThrowawayStores::new("datasets-dry-run");
    copy_the_samples_to(&stores.config().content_folder);
    let cx = support::context_with(stores, &StandInLlm::signed_out(), StandInEmbedder::default);
    let held_out = support::document_of(&support::sample_chapter(support::SAMPLE_PAGES));
    let dry = DatasetRequest {
        name: "smoke".to_owned(),
        holdout: vec![held_out],
        dry_run: true,
        ..DatasetRequest::default()
    };

    let (progress, finished) = built(&cx, &dry).await;

    let stages: Vec<DatasetStage> = progress.iter().map(|progress| progress.stage).collect();
    let mut expected: Vec<DatasetStage> = (0..=3)
        .map(|done| DatasetStage::Exporting { done, total: 3 })
        .collect();
    expected.extend([
        DatasetStage::Checking,
        DatasetStage::Planning,
        DatasetStage::WritingManifest,
    ]);
    assert_eq!(stages, expected);
    assert!(
        progress.iter().all(|progress| progress.spent.tokens() == 0),
        "a dry run asks no model: {progress:?}"
    );
    let report = finished.expect("a dry run asks no model, so a signed-out one does not stop it");
    assert!(report.folder.is_absolute(), "{:?}", report.folder);
    assert!(report.folder.ends_with("smoke"), "{:?}", report.folder);
    assert!(report.folder.join("cpt/raw.jsonl").is_file());
    assert!(report.folder.join("manifest.json").is_file());
    assert_eq!(report.documents, 3);
    assert!(report.sections_heldout > 0, "{report:#?}");
    assert!(report.planned_calls.is_some(), "{report:#?}");

    let paid = DatasetRequest {
        dry_run: false,
        ..dry
    };

    let (progress, finished) = built(&cx, &paid).await;

    assert!(progress.is_empty(), "nothing is told: {progress:?}");
    let failure = finished.expect_err("a paid build over a signed-out model");
    assert_eq!(failure.kind, FailureKind::ClaudeSignedOut, "{failure:#?}");
}

#[tokio::test]
async fn a_graph_that_is_down_fails_the_build_with_the_graph_named() {
    let folder = tempfile::tempdir().expect("a temporary folder should be made");
    let config = Config {
        content_folder: folder.path().join("content"),
        dataset_folder: folder.path().join("datasets"),
        ..support::closed_ports_config()
    };
    copy_the_samples_to(&config.content_folder);
    let services = support::StandInsOnClosedPorts {
        pages: Arc::new(StubServices::new(Scenario::SampleChapter)),
    };
    let cx = LiveContext::new(config, services);
    let wanted = DatasetRequest {
        name: "never-built".to_owned(),
        dry_run: true,
        ..DatasetRequest::default()
    };

    let (progress, finished) = built(&cx, &wanted).await;

    assert!(progress.is_empty(), "nothing is told: {progress:?}");
    let failure = finished.expect_err("a build over a graph that nothing listens on");
    assert_eq!(failure.kind, FailureKind::FalkorDbDown, "{failure:#?}");
    assert!(
        !folder.path().join("datasets/never-built").exists(),
        "a build that did not start writes nothing"
    );
}
