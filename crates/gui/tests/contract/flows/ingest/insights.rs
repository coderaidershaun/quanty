//! The end of an ingest says what became of the insights, takeaways and cheat sheet: that they are
//! written, that they were saved already, that a part of a book gets none, or why they could not
//! be written and which command writes them.

use contract::{
    Category, ChapterInsights, DocId, DocumentName, IngestOutcome, IngestRequest, InsightsOutcome,
    ModelTokens, Usage,
};
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use gui::app::layout::DEFAULT_WINDOW;
use gui::panels::ingest;
use gui::state::{IngestJob, Shared};
use gui::testkit::{self, Host};
use uuid::Uuid;

use super::{open, pick};
use crate::flows::{click, copied_by, has, node, says, shared};

#[test]
fn the_end_of_an_ingest_says_what_became_of_the_insights() {
    let (mut harness, _) = open("ingest-ready", DEFAULT_WINDOW);
    pick(&mut harness, "index.pdf");
    click(&mut harness, Role::Button, "Other part");

    click(&mut harness, Role::Button, "Start ingest");

    assert!(says(
        &harness,
        "A part of a book that is not a chapter gets no insights."
    ));
    assert!(!says(&harness, "are written"));
    assert!(!has(&harness, Role::Button, "Copy command"));

    let (mut harness, _) = open("ingest-insights-failed", DEFAULT_WINDOW);

    click(&mut harness, Role::Button, "Start ingest");

    let IngestJob::Finished {
        result: Ok(IngestOutcome::Ingested(report)),
        ..
    } = &shared(&harness).ingest
    else {
        panic!(
            "the ingest did not end stored: {:?}",
            shared(&harness).ingest
        );
    };
    let command = ChapterInsights::command_for(report.doc);
    assert!(says(
        &harness,
        "Ingested Option Volatility and Pricing, chapter 3: Greeks"
    ));
    assert!(says(&harness, "The insights were not written"));
    assert!(says(&harness, "Why: claude reached its usage limit."));
    assert!(says(&harness, "The items are stored"));
    assert!(says(&harness, &command));
    assert!(!says(&harness, "are written"));
    testkit::save_png(&mut harness, "app-ingest-insights-failed");
    let copied = copied_by(&mut harness, |harness| {
        node(harness, Role::Button, "Copy command").click();
    });
    assert_eq!(copied, [command]);
}

#[test]
fn a_start_of_an_ingested_document_says_what_its_insights_cost() {
    let request = IngestRequest {
        pdf: contract::UploadId::default(),
        file_name: "chapter-3-greeks.pdf".into(),
        media: "Option Volatility and Pricing".to_owned(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 3,
            name: "Greeks".to_owned(),
        },
        tags: Vec::new(),
    };
    let ended = |insights, usage| IngestJob::Finished {
        spent: Usage::default(),
        request: request.clone(),
        result: Ok(IngestOutcome::AlreadyIngested {
            doc: DocId(Uuid::from_u128(9)),
            artifacts: 0,
            items: 120,
            insights: Some(insights),
            usage,
        }),
    };
    let mut local = ingest::Local::default();
    let mut harness = testkit::panel([720.0, 600.0], Shared::default(), move |ui, cx| {
        ingest::show(ui, &mut local, cx);
    });
    let sonnet = ModelTokens {
        model: "claude-sonnet-5-5".to_owned(),
        input: 12_000,
        output: 4_000,
        ..ModelTokens::default()
    };
    let written = Usage {
        models: vec![sonnet],
        cost_usd: Some(0.14),
    };
    let nothing_asked = Usage {
        models: Vec::new(),
        cost_usd: Some(0.0),
    };

    harness.state_mut().shared.ingest = ended(InsightsOutcome::Written, written);
    harness.run_ok();

    let said = |harness: &Harness<'static, Host>, words: &str| {
        harness.query_all_by_label_contains(words).next().is_some()
    };
    assert!(said(
        &harness,
        "120 items are stored with 0 artifact descriptions. Nothing was converted or embedded."
    ));
    assert!(said(
        &harness,
        "The insights, takeaways and cheat sheet are written"
    ));
    assert!(said(
        &harness,
        "Tokens: claude-sonnet-5-5 16k. Cost ≈ $0.14."
    ));

    harness.state_mut().shared.ingest = ended(InsightsOutcome::AlreadySaved, nothing_asked);
    harness.run_ok();

    assert!(said(
        &harness,
        "The insights were saved already, so nothing was asked for them."
    ));
    assert!(!said(&harness, "Tokens:"));
    assert!(!said(&harness, "Cost"));
    for (missing_artifacts, missing_insights, expected) in [
        (
            true,
            false,
            "A start writes the missing artifact descriptions.",
        ),
        (
            true,
            true,
            "A start writes the missing artifact descriptions and insights.",
        ),
    ] {
        harness.state_mut().shared.ingest = IngestJob::Checked {
            request: request.clone(),
            preflight: contract::Preflight {
                name: request.name.clone(),
                pages: Some(12),
                state: Some(contract::ChapterState::IngestedWithoutMetadata {
                    items: 120,
                    missing_artifacts,
                    missing_insights,
                }),
                blockers: Vec::new(),
            },
        };
        harness.run_ok();
        assert!(said(&harness, expected));
        assert!(!said(&harness, "true"));
        assert!(!said(&harness, "false"));
    }
    if let IngestJob::Checked { preflight, .. } = &mut harness.state_mut().shared.ingest {
        preflight.state = Some(contract::ChapterState::Converted);
    }
    harness.run_ok();
    assert!(said(&harness, "missing artifact descriptions and insights"));
}

pub(super) fn assert_artifacts_precede_store_changes() {
    use backend::{Handler, Reply, fake::Fake};
    use contract::{Command, Event, IngestStage, RequestId};
    let (reply, events, _stop) = Reply::collecting();
    let fake = Fake::scene(
        "ingest-ready",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap()
    .instant();
    let command = Command::Ingest {
        request: RequestId(1),
        ingest: IngestRequest {
            pdf: contract::UploadId::default(),
            file_name: "chapter.pdf".into(),
            media: "Sample".into(),
            category: Category::Book,
            name: DocumentName::Title("Sample".into()),
            tags: Vec::new(),
        },
    };
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(fake.serve(command, reply));
    let stages: Vec<_> = events
        .try_iter()
        .filter_map(|event| match event {
            Event::IngestProgress { progress, .. } => Some(progress.stage),
            _ => None,
        })
        .collect();
    let metadata = stages
        .iter()
        .rposition(|stage| matches!(stage, IngestStage::WritingArtifacts { .. }))
        .unwrap();
    let graph = stages
        .iter()
        .position(|stage| matches!(stage, IngestStage::WritingGraph))
        .unwrap();
    assert!(
        metadata < graph,
        "fake progress must show required metadata before store replacement"
    );
}
