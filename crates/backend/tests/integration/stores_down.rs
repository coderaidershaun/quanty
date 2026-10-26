//! Checks that every command over stores that nothing listens on answers once, with the store
//! named, and that nothing is converted, paid for or written.

use std::sync::Arc;

use backend::live::LiveContext;
use contract::{
    Command, DatasetRequest, DocId, DocumentTagsEdit, Event, Failure, FailureKind, IngestRequest,
    MediaEdit, NewMedia, RequestId, Service, ServiceState,
};
use ocr::testing::{Scenario, StubServices, sample_pdf};
use rag_core::Config;
use uuid::Uuid;

use crate::ingest::{assert_consumed, request};
use crate::support::{self, StandInsOnClosedPorts, sample_chapter, served};

const REQUEST: RequestId = RequestId(7);
const STORES: [FailureKind; 2] = [FailureKind::FalkorDbDown, FailureKind::QdrantDown];

#[tokio::test(flavor = "multi_thread")]
async fn every_command_answers_once_with_the_store_named_while_the_stores_are_down() {
    let home = tempfile::tempdir().expect("a temporary folder should be made");
    let config = Config {
        content_folder: home.path().join("content"),
        dataset_folder: home.path().join("datasets"),
        ..support::closed_ports_config()
    };
    let pages = Arc::new(StubServices::new(Scenario::SampleChapter));
    let services = StandInsOnClosedPorts {
        pages: Arc::clone(&pages),
    };
    let cx = LiveContext::new(config, services).with_home(home.path().to_path_buf());
    let wanted = request(&cx);
    let doc = DocId(Uuid::from_u128(1));
    let hedging = "Dynamic Hedging".to_owned();
    let commands = [
        Command::LoadCatalogue { request: REQUEST },
        Command::DeleteDocument {
            request: REQUEST,
            doc,
        },
        Command::DeleteMedia {
            request: REQUEST,
            title: "Quanty Sample Notes".to_owned(),
        },
        Command::SetDocumentTags {
            request: REQUEST,
            edit: DocumentTagsEdit {
                doc,
                add: vec!["options".to_owned()],
                ..DocumentTagsEdit::default()
            },
        },
        Command::SaveMedia {
            request: REQUEST,
            media: NewMedia {
                title: hedging.clone(),
                ..NewMedia::default()
            },
        },
        Command::EditMedia {
            request: REQUEST,
            edit: MediaEdit {
                title: hedging,
                ..MediaEdit::default()
            },
        },
        Command::BuildDataset {
            request: REQUEST,
            build: DatasetRequest {
                name: "never-built".to_owned(),
                dry_run: true,
                ..DatasetRequest::default()
            },
        },
        // The same receipt is checked and started twice: the PDF it names is kept for the retry.
        Command::Preflight {
            request: REQUEST,
            ingest: wanted.clone(),
        },
        Command::Ingest {
            request: REQUEST,
            ingest: wanted.clone(),
        },
        Command::Preflight {
            request: REQUEST,
            ingest: wanted.clone(),
        },
        Command::Ingest {
            request: REQUEST,
            ingest: wanted.clone(),
        },
    ];
    for command in commands {
        let events = served(&cx, command.clone()).await;
        let [event] = events.as_slice() else {
            panic!("{command:?} must answer once: {events:#?}");
        };
        let failure = failure_in(event).unwrap_or_else(|| panic!("{command:?}: {event:#?}"));
        assert!(STORES.contains(&failure.kind), "{command:?}: {failure:?}");
    }
    assert_consumed(&cx, &wanted, &sample_pdf());
    assert!(
        !home.path().join("datasets/never-built").exists(),
        "a build that did not start writes nothing"
    );
    for file_name in ["../outside.pdf", "folder\\outside.pdf"] {
        let invalid = IngestRequest {
            file_name: file_name.into(),
            ..wanted.clone()
        };
        let command = Command::Preflight {
            request: REQUEST,
            ingest: invalid,
        };
        let events = served(&cx, command).await;
        let failure = events.first().and_then(failure_in);
        assert_eq!(
            failure.map(|failure| failure.kind),
            Some(FailureKind::BadFile)
        );
    }
    assert_eq!(pages.calls(), [], "no page was converted");

    the_health_check_names_both_stores(&cx).await;
    a_page_loads_from_disk_and_its_concepts_name_the_graph(&cx).await;
}

async fn the_health_check_names_both_stores(cx: &LiveContext<StandInsOnClosedPorts>) {
    let events = served(cx, Command::CheckHealth { request: REQUEST }).await;
    for (service, kind) in [
        (Service::Qdrant, FailureKind::QdrantDown),
        (Service::FalkorDb, FailureKind::FalkorDbDown),
    ] {
        let state = events.iter().find_map(|event| match event {
            Event::Health {
                service: checked,
                state,
                ..
            } if *checked == service => Some(state),
            _ => None,
        });
        assert!(
            matches!(state, Some(ServiceState::Down(failure)) if failure.kind == kind),
            "{service:?}: {state:?}"
        );
    }
}

async fn a_page_loads_from_disk_and_its_concepts_name_the_graph(
    cx: &LiveContext<StandInsOnClosedPorts>,
) {
    let chapter = sample_chapter(support::SAMPLE_PAGES);
    let command = Command::LoadPage {
        request: REQUEST,
        doc: DocId::default(),
        page: 5,
        folder: Some(chapter),
    };
    let events = served(cx, command).await;
    let [
        Event::Page {
            result: Ok(page), ..
        },
        Event::PageConcepts {
            result: Err(failure),
            ..
        },
    ] = events.as_slice()
    else {
        panic!("expected the page and the failure of its concepts: {events:#?}");
    };
    assert_eq!(page.page, 5, "the page needs no store");
    assert_eq!(failure.kind, FailureKind::FalkorDbDown);
}

/// The failure that a command's one answer carries, if it carries one.
fn failure_in(event: &Event) -> Option<&Failure> {
    match event {
        Event::Catalogue { result, .. } => result.as_ref().err(),
        Event::DocumentDeleted { result, .. }
        | Event::MediaDeleted { result, .. }
        | Event::DocumentTagsSaved { result, .. }
        | Event::MediaSaved { result, .. }
        | Event::MediaEdited { result, .. } => result.as_ref().err(),
        Event::DatasetFinished { result, .. } => result.as_ref().err(),
        Event::Preflight { result, .. } => result.as_ref().err(),
        Event::IngestFinished { result, .. } => result.as_ref().err(),
        _ => None,
    }
}
