//! Checks that the live backend switches to another database only after both of its stores
//! answered, and that a refused switch leaves the database it had.

use gui::backend::live::{LiveContext, Services};
use gui::contract::{Command, Database, DocId, Event, Failure, FailureKind, RequestId};
use rag_ingestion::ingest_chapter;
use rag_ingestion::testing::{StandInLlm, ThrowawayStores, chapter_at};

use crate::support::{self, IN_DEPTH, copy_folder, sample_chapter, served};

const REQUEST: RequestId = RequestId(7);

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored database::"]
async fn using_a_database_connects_to_its_stores_before_it_swaps_and_keeps_the_old_ones_on_failure()
{
    let stores = ThrowawayStores::new("using-a-database");
    let chapter = stores.config().content_folder.join(IN_DEPTH);
    copy_folder(&sample_chapter(IN_DEPTH), &chapter);
    let connected = stores.connect().await;
    let models = stores.models(StandInLlm::finding_nothing());
    let stored = ingest_chapter(chapter_at(&chapter), &models, &connected).await;
    let stored = stored.expect("the chapter should be ingested");
    let throwaway = Database {
        name: "Throwaway".to_owned(),
        ..stores.config().local_database()
    };
    let (cx, _pages) = support::closed_with_stub_pages(&stores.temporary_folder().join("none"));

    let switched = switch_to(&cx, throwaway.clone()).await;
    assert_eq!(switched, Ok(()), "stores that answer should be used");
    assert_eq!(documents_of(&cx).await, Ok(vec![stored.doc_id.into()]));

    let mount = stores.temporary_folder().join("mount");
    std::fs::create_dir(&mount).expect("a folder should be made");
    let unreachable = Database {
        name: "Server".to_owned(),
        content_folder: mount,
        ..support::closed_ports_config().local_database()
    };
    let refused = switch_to(&cx, unreachable)
        .await
        .expect_err("nothing answers there");
    assert_eq!(refused.kind, FailureKind::QdrantDown);
    assert!(
        refused.hint.contains("http://127.0.0.1:1"),
        "the hint should name the address: {}",
        refused.hint
    );
    assert_eq!(cx.config().falkordb_graph, throwaway.falkordb_graph);
    assert_eq!(documents_of(&cx).await, Ok(vec![stored.doc_id.into()]));

    let unmounted = Database {
        name: "Mount".to_owned(),
        content_folder: stores.temporary_folder().join("not-mounted"),
        ..throwaway.clone()
    };
    let refused = switch_to(&cx, unmounted)
        .await
        .expect_err("the folder is not there");
    assert_eq!(refused.kind, FailureKind::SourceMissing);
    assert!(
        refused.hint.contains("not-mounted"),
        "the hint should name the folder: {}",
        refused.hint
    );
    assert_eq!(cx.config().content_folder, throwaway.content_folder);
}

async fn switch_to<S: Services>(cx: &LiveContext<S>, database: Database) -> Result<(), Failure> {
    let command = Command::UseDatabase {
        request: REQUEST,
        database,
    };
    match served(cx, command).await.as_slice() {
        [Event::DatabaseUsed { request, result }] if *request == REQUEST => result.clone(),
        other => panic!("expected one answer to the switch, for {REQUEST:?}: {other:#?}"),
    }
}

async fn documents_of<S: Services>(cx: &LiveContext<S>) -> Result<Vec<DocId>, Failure> {
    let command = Command::LoadCatalogue { request: REQUEST };
    match served(cx, command).await.as_slice() {
        [Event::Catalogue { request, result }] if *request == REQUEST => {
            result.clone().map(|catalogue| {
                catalogue
                    .media
                    .iter()
                    .flat_map(|media| media.documents.iter().map(|document| document.id))
                    .collect()
            })
        }
        other => panic!("expected one catalogue, for {REQUEST:?}: {other:#?}"),
    }
}
