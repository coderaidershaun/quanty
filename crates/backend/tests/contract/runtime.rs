//! Completion releases a transport request even when its handler fails or stops.
use backend::{Handler, Reply, Runtime};
use contract::{Command, Event, Failure, RequestId};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Outcomes;
impl Handler for Outcomes {
    async fn serve(&self, command: Command, reply: Reply) {
        let request = command.request();
        match request.0 {
            1 => reply.send(Event::Catalogue {
                request,
                result: Ok(Default::default()),
            }),
            2 => {
                for event in command.failed(&Failure::internal("failed")) {
                    reply.send(event);
                }
            }
            3 => panic!("handler panic"),
            4 => std::future::pending::<()>().await,
            5 => reply.cancelled().await,
            _ => {}
        }
    }
}

#[tokio::test]
async fn requests_finish_on_success_failure_panic_and_cancel() {
    fn assert_send<T: Send>() {}
    assert_send::<Runtime<Outcomes>>();
    let wakes = Arc::new(AtomicUsize::new(0));
    let counted = wakes.clone();
    let mut runtime = Runtime::on(
        tokio::runtime::Handle::current(),
        Arc::new(Outcomes),
        move || {
            counted.fetch_add(1, Ordering::Relaxed);
        },
    );
    for id in 1..=4 {
        runtime.send(Command::LoadCatalogue {
            request: RequestId(id),
        });
    }
    runtime.send(Command::Ingest {
        request: RequestId(5),
        ingest: contract::IngestRequest {
            pdf: Default::default(),
            file_name: "chapter.pdf".into(),
            media: "Book".into(),
            category: contract::Category::Book,
            name: contract::DocumentName::Title("Chapter".into()),
            tags: Vec::new(),
        },
    });
    runtime.send(Command::Cancel(RequestId(4)));
    runtime.send(Command::Cancel(RequestId(5)));
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !runtime.is_idle() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut finished = runtime.take_finished();
    finished.sort();
    assert_eq!(finished, (1..=5).map(RequestId).collect::<Vec<_>>());
    assert!(runtime.take_finished().is_empty());
    let events: Vec<_> = std::iter::from_fn(|| runtime.try_next()).collect();
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Catalogue {
            request: RequestId(1),
            result: Ok(_)
        }
    )));
    for id in [2, 3] {
        assert!(events.iter().any(
            |event| matches!(event, Event::Catalogue { request, result: Err(_) } if request.0 == id)
        ));
    }
    assert!(wakes.load(Ordering::Relaxed) >= 5);
    tokio::task::spawn_blocking(|| {
        let mut own = Runtime::start_own_threads(Outcomes, || {}).unwrap();
        own.send(Command::LoadCatalogue {
            request: RequestId(1),
        });
        while !own.is_idle() {
            std::thread::yield_now();
        }
        assert_eq!(own.take_finished(), vec![RequestId(1)]);
    })
    .await
    .unwrap();
}
