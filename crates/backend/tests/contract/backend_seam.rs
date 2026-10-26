//! Checks that every scene of the fake backend opens in the state it is named for, and that the
//! fake loads every sample page.

use backend::fake::{self, Fake};
use contract::{DatasetStage, DocumentName, FailureKind, Loadable, Tab};
use egui_kittest::Harness;
use gui::app::App;
use gui::app::layout::DEFAULT_WINDOW;
use gui::state::{DatasetJob, IngestJob, Shared};
use gui::testkit::{self, run_until};

use super::sample_pages;

fn fake_of(scene: &str) -> Fake {
    Fake::scene(scene, &testkit::samples_folder())
        .unwrap_or_else(|error| panic!("scene `{scene}` does not start: {error}"))
        .instant()
}

fn is_failed<T>(slot: &Loadable<T>, kind: FailureKind) -> bool {
    slot.failure().is_some_and(|failure| failure.kind == kind)
}

fn assert_scene(name: &str, shared: &Shared) {
    let (ask, source) = (&shared.ask, &shared.source);
    let documents = shared
        .library
        .catalogue
        .ready()
        .map(|found| found.documents().count());
    let results = ask.search.ready().map(|reply| reply.results.len());
    let nodes = ask.graph.ready().map(|graph| graph.nodes.len());
    let blocks = ask.answer.ready().map(|answer| answer.blocks.len());
    let trace = ask.search.ready().map(|reply| &reply.trace);
    // With no seed the search stops at its first step, so no later step has a value.
    let stopped_at_the_first_step = trace.is_some_and(|trace| *trace == Default::default());
    let ok = match name {
        "idle" | "gallery" | "balances-unread" => {
            documents == Some(5) && ask.search == Loadable::Idle
        }
        "first-run" | "empty-library" => documents == Some(0) && ask.search == Loadable::Idle,
        "black-scholes" => results == Some(9) && nodes > Some(0) && blocks > Some(0),
        "no-answer" => results == Some(9) && blocks == Some(0),
        "results-only" => results == Some(9) && nodes > Some(0) && ask.answer == Loadable::Idle,
        "searching" => ask.search.is_loading(),
        "answering" => results == Some(9) && ask.answer.is_loading(),
        "no-sources" => results == Some(0) && stopped_at_the_first_step,
        "search-failed" => is_failed(&ask.search, FailureKind::EmbeddingFailed),
        "out-of-credit" => is_failed(&ask.search, FailureKind::EmbeddingOutOfCredit),
        "answer-failed" => {
            results == Some(9) && is_failed(&ask.answer, FailureKind::ClaudeUsageLimit)
        }
        "graph-failed" => is_failed(&ask.graph, FailureKind::FalkorDbDown) && blocks > Some(0),
        "no-labels" => {
            results == Some(0) && trace.is_some_and(|trace| trace.documents_searched == Some(0))
        }
        "no-concepts" => {
            let no_seeds = trace.is_some_and(|trace| trace.seed_concepts == Some(Vec::new()));
            results == Some(9) && nodes == Some(0) && blocks > Some(0) && no_seeds
        }
        "source-missing" => {
            let is_empty = source.concepts == Loadable::Ready(Vec::new());
            results == Some(9) && is_failed(&source.page, FailureKind::SourceMissing) && is_empty
        }
        "stores-down" => {
            is_failed(&ask.search, FailureKind::QdrantDown)
                && is_failed(&shared.library.catalogue, FailureKind::QdrantDown)
                && source.page.ready().is_some()
                && is_failed(&source.concepts, FailureKind::FalkorDbDown)
        }
        "ingest-ready" | "ingest-failed" | "ingest-insights-failed" | "ingest-running" => {
            let is_checked = matches!(
                &shared.ingest,
                IngestJob::Checked { preflight, .. }
                    if matches!(preflight.name, DocumentName::Chapter { number: 3, .. })
                        && preflight.blockers.is_empty()
            );
            shared.tab == Tab::Ingest && is_checked
        }
        "datasets-ready" => shared.tab == Tab::Datasets && shared.datasets == DatasetJob::Idle,
        "datasets-running" => shared.tab == Tab::Datasets && is_at_the_stall_of_its_build(shared),
        "datasets-failed" => {
            let failure = match &shared.datasets {
                DatasetJob::Finished {
                    result: Err(failure),
                    ..
                } => Some(failure.kind),
                _ => None,
            };
            shared.tab == Tab::Datasets && failure == Some(FailureKind::ClaudeUsageLimit)
        }
        other => panic!("the test has no expectation for the scene `{other}`"),
    };
    assert!(ok, "scene `{name}` opened in the wrong state: {shared:#?}");
}

fn is_at_the_stall_of_its_build(shared: &Shared) -> bool {
    matches!(
        &shared.datasets,
        DatasetJob::Running { progress: Some(progress), .. }
            if progress.stage == DatasetStage::WritingSynthetic { done: 6, total: 12 }
    )
}

fn open_scene(name: &str, rests: bool) -> Harness<'static, App> {
    let mut harness = testkit::app(name, DEFAULT_WINDOW);
    if rests {
        testkit::settle(&mut harness);
        return harness;
    }
    // A scene that never rests is looked at once the state it shows has arrived.
    let (what, arrived): (&str, fn(&Shared) -> bool) = match name {
        "answering" => ("the results", |shared| shared.ask.search.ready().is_some()),
        "gallery" => ("the catalogue", |shared| {
            shared.library.catalogue.ready().is_some()
        }),
        "ingest-running" => ("the check", |shared| {
            matches!(shared.ingest, IngestJob::Checked { .. })
        }),
        "datasets-running" => ("the build at its stall", is_at_the_stall_of_its_build),
        _ => ("the ask to start", |shared| shared.ask.is_running()),
    };
    run_until(&mut harness, what, arrived);
    harness
}

#[test]
fn every_scene_opens_and_every_sample_page_loads() {
    assert_eq!(fake::scenes().len(), 26);
    for scene in fake::scenes() {
        let mut harness = open_scene(scene.name, scene.rests);
        assert_scene(scene.name, harness.state().shared());
        testkit::save_png(&mut harness, &format!("scene-{}", scene.name));
    }

    sample_pages::assert_every_sample_page_loads(&fake_of("idle"));
}
