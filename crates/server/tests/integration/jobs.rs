//! Paid jobs belong to the server: they outlive the tab that started them, one runs at a time, and
//! a reloaded tab follows a running ingest and can stop it.

use super::support::{TestServer, configured_with_mcp, finish, frame, send, test_server};
use contract::{
    Category, Command, DatasetRequest, DocumentName, Event, Frame, IngestOutcome, IngestRequest,
    RequestId, StartupFacts, UploadId,
};

async fn running(server: &TestServer, expected: bool) -> StartupFacts {
    let client = server.client();
    let cookie = server.login(&client).await;
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let facts: StartupFacts = client
                .get(server.url("/api/facts"))
                .header("cookie", &cookie)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            if !facts.running.is_empty() == expected {
                break facts;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
}
fn dataset(request: u64) -> Command {
    Command::BuildDataset {
        request: RequestId(request),
        build: DatasetRequest {
            name: "guarded".into(),
            ..Default::default()
        },
    }
}

fn follow(request: u64) -> Command {
    Command::FollowIngest {
        request: RequestId(request),
    }
}

fn sample_ingest() -> IngestRequest {
    IngestRequest {
        pdf: UploadId::default(),
        file_name: "sample.pdf".into(),
        media: "Sample".into(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 1,
            name: "Sample".into(),
        },
        tags: vec![],
    }
}

#[tokio::test]
async fn a_reloaded_tab_follows_the_running_ingest_and_stops_it() {
    // The server gets its own background slot, so no other test of this binary can hold it.
    let server =
        configured_with_mcp(Some("secret"), |_, _| {}, mcp::BackgroundJobs::new(), None).await;
    let ingest = sample_ingest();
    let mut starter = server.socket().await;
    send(
        &mut starter,
        &Command::Ingest {
            request: RequestId(7),
            ingest: ingest.clone(),
        },
    )
    .await;
    assert!(matches!(
        frame(&mut starter).await,
        Frame::Event(Event::IngestProgress { .. })
    ));

    let mut reloaded = server.socket().await;
    send(&mut reloaded, &follow(1)).await;
    let Frame::Event(Event::IngestFollowed {
        request,
        ingest: followed,
        progress,
    }) = frame(&mut reloaded).await
    else {
        panic!("the reloaded tab was not given the running ingest");
    };
    assert_eq!(request, RequestId(1));
    assert_eq!(followed, ingest);
    assert!(progress.is_some(), "the ingest had made a step already");
    assert!(matches!(
        frame(&mut reloaded).await,
        Frame::Event(Event::IngestProgress {
            request: RequestId(1),
            ..
        })
    ));

    send(&mut reloaded, &Command::Cancel(RequestId(1))).await;
    assert!(matches!(
        finish(&mut reloaded, RequestId(1)).await.last(),
        Some(Event::IngestFinished {
            request: RequestId(1),
            result: Ok(IngestOutcome::Cancelled),
        })
    ));
    assert!(matches!(
        finish(&mut starter, RequestId(7)).await.last(),
        Some(Event::IngestFinished {
            request: RequestId(7),
            result: Ok(IngestOutcome::Cancelled),
        })
    ));
    running(&server, false).await;

    // The next ingest starts, and its tab leaves before the end.
    send(
        &mut reloaded,
        &Command::Ingest {
            request: RequestId(2),
            ingest,
        },
    )
    .await;
    assert!(matches!(
        frame(&mut reloaded).await,
        Frame::Event(Event::IngestProgress { .. })
    ));
    reloaded.close(None).await.unwrap();
    running(&server, false).await;

    let mut returned = server.socket().await;
    send(&mut returned, &follow(3)).await;
    assert!(matches!(
        &finish(&mut returned, RequestId(3)).await[..],
        [
            Event::IngestFollowed {
                request: RequestId(3),
                ..
            },
            Event::IngestFinished {
                request: RequestId(3),
                result: Ok(IngestOutcome::Ingested(_)),
            },
        ]
    ));
    send(&mut returned, &follow(4)).await;
    assert!(
        finish(&mut returned, RequestId(4)).await.is_empty(),
        "an end is shown once, and then there is nothing to follow"
    );
}

#[tokio::test]
async fn paid_jobs_are_global_detached_and_only_the_owner_can_cancel() {
    let server = test_server(Some("secret")).await;
    let mut owner = server.socket().await;
    let alternate = test_server(Some("secret")).await;
    let mut other = alternate.socket().await;
    send(&mut owner, &dataset(11)).await;
    assert!(matches!(
        frame(&mut owner).await,
        Frame::Event(Event::DatasetProgress { .. })
    ));
    let facts = running(&server, true).await;
    assert_eq!(facts.running.len(), 1);
    assert_eq!(facts.running[0].kind, contract::JobKind::Dataset);
    assert_eq!(facts.running[0].name, "guarded");
    send(&mut other, &Command::Cancel(RequestId(11))).await;
    send(&mut other, &dataset(12)).await;
    let refused = finish(&mut other, RequestId(12)).await;
    assert!(
        matches!(&refused[..], [Event::DatasetFinished { result: Err(failure), .. }] if failure.detail == "an ingest is running" && failure.hint.contains("guarded"))
    );
    owner.close(None).await.unwrap();
    running(&server, true).await;
    running(&server, false).await;

    send(&mut other, &dataset(13)).await;
    assert!(matches!(
        frame(&mut other).await,
        Frame::Event(Event::DatasetProgress { .. })
    ));
    send(&mut other, &Command::Cancel(RequestId(13))).await;
    finish(&mut other, RequestId(13)).await;
    running(&server, false).await;

    let ingest = sample_ingest();
    send(
        &mut other,
        &Command::Ingest {
            request: RequestId(14),
            ingest: ingest.clone(),
        },
    )
    .await;
    assert!(matches!(
        frame(&mut other).await,
        Frame::Event(Event::IngestProgress { .. })
    ));
    send(&mut other, &Command::Cancel(RequestId(14))).await;
    assert!(matches!(
        &finish(&mut other, RequestId(14)).await[..],
        [
            ..,
            Event::IngestFinished {
                result: Ok(IngestOutcome::Cancelled),
                ..
            }
        ]
    ));
    running(&server, false).await;
    other.close(None).await.unwrap();
    let mut third = server.socket().await;
    send(
        &mut third,
        &Command::Ingest {
            request: RequestId(14),
            ingest,
        },
    )
    .await;
    assert!(matches!(
        finish(&mut third, RequestId(14)).await.last(),
        Some(Event::IngestFinished {
            result: Ok(IngestOutcome::Ingested(_)),
            ..
        })
    ));
    running(&server, false).await;
    send(&mut third, &dataset(15)).await;
    let events = finish(&mut third, RequestId(15)).await;
    assert!(matches!(
        events.last(),
        Some(Event::DatasetFinished { result: Ok(_), .. })
    ));
}
