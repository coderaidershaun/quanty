//! Socket replies retain request identity and paid jobs survive their originating tab.

use super::support::{TestServer, finish, frame, send, test_server};
use contract::{
    Category, Command, DatasetRequest, DocumentName, Event, Frame, IngestRequest, RequestId,
    StartupFacts, UploadId,
};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

#[tokio::test]
async fn websocket_events_finish_and_bad_commands_close_the_connection() {
    let local = test_server(None).await;
    let client = local.client();
    for name in ["localhost", "127.0.0.1", "127.0.0.2", "[::1]"] {
        assert_eq!(
            client
                .get(local.url("/api/facts"))
                .header("host", format!("{name}:{}", local.address.port()))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    for host in [
        format!("attacker.example:{}", local.address.port()),
        format!("localhost.attacker.example:{}", local.address.port()),
        format!("localhost:{}", local.address.port().wrapping_add(1)),
        format!("attacker@127.0.0.1:{}", local.address.port()),
    ] {
        assert_eq!(
            client
                .get(local.url("/api/facts"))
                .header("host", &host)
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    for headers in [
        vec![],
        vec![("origin", format!("http://{}", local.address))],
        vec![
            ("host", format!("localhost:{}", local.address.port())),
            (
                "origin",
                format!("http://LOCALHOST:{}", local.address.port()),
            ),
        ],
        vec![
            ("host", format!("[::1]:{}", local.address.port())),
            (
                "origin",
                format!("http://[0:0:0:0:0:0:0:1]:{}", local.address.port()),
            ),
        ],
        vec![
            ("origin", format!("http://{}", local.address)),
            ("x-forwarded-proto", "https".into()),
        ],
    ] {
        let headers: Vec<_> = headers
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        handshake(&local, &headers)
            .await
            .unwrap()
            .close(None)
            .await
            .unwrap();
    }
    for origin in [
        "https://attacker.example".into(),
        "null".into(),
        format!("https://{}", local.address),
        format!("http://{}/", local.address),
        format!("http://{}?query", local.address),
        format!("http://{}#fragment", local.address),
        format!("http://user@{}", local.address),
        format!("http://{}, http://{}", local.address, local.address),
    ] {
        let error = handshake(&local, &[("origin", &origin)])
            .await
            .expect_err("unsafe origin must be refused");
        assert!(
            matches!(error, tokio_tungstenite::tungstenite::Error::Http(response) if response.status() == 403),
            "{origin}"
        );
    }
    let error = handshake(
        &local,
        &[
            ("host", "attacker.example"),
            ("origin", "http://attacker.example"),
        ],
    )
    .await
    .expect_err("rebinding must be refused");
    assert!(
        matches!(error, tokio_tungstenite::tungstenite::Error::Http(response) if response.status() == 403)
    );
    let server = test_server(Some("secret")).await;
    let cookie = server.login(&server.client()).await;
    for headers in [
        vec![
            ("host", "quanty.example.com"),
            ("origin", "https://QUANTY.example.com:443"),
            ("x-forwarded-proto", "https"),
        ],
        vec![
            ("host", "quanty.example.com:443"),
            ("origin", "https://quanty.example.com"),
            ("x-forwarded-proto", "https"),
        ],
        vec![
            ("host", "quanty.example.com"),
            ("origin", "http://quanty.example.com:80"),
        ],
        vec![("host", "quanty.example.com")],
    ] {
        let mut headers = headers;
        headers.push(("cookie", &cookie));
        handshake(&server, &headers)
            .await
            .unwrap()
            .close(None)
            .await
            .unwrap();
    }
    for headers in [
        vec![("origin", "https://attacker.example")],
        vec![
            ("host", "quanty.example.com"),
            ("origin", "https://quanty.example.com"),
        ],
        vec![
            ("host", "quanty.example.com"),
            ("origin", "http://quanty.example.com"),
            ("x-forwarded-proto", "https"),
        ],
        vec![
            ("host", "quanty.example.com"),
            ("origin", "https://quanty.example.com:444"),
            ("x-forwarded-proto", "https"),
        ],
        vec![
            ("origin", "http://localhost"),
            ("origin", "http://localhost"),
        ],
        vec![("x-forwarded-proto", "https,http")],
        vec![
            ("x-forwarded-proto", "https"),
            ("x-forwarded-proto", "https"),
        ],
    ] {
        let mut headers = headers;
        headers.push(("cookie", &cookie));
        let error = handshake(&server, &headers)
            .await
            .expect_err("unsafe upgrade must be refused");
        assert!(
            matches!(error, tokio_tungstenite::tungstenite::Error::Http(response) if response.status() == 403)
        );
    }
    let mut first = server.socket().await;
    let mut second = server.socket().await;
    for socket in [&mut first, &mut second] {
        send(
            socket,
            &Command::LoadCatalogue {
                request: RequestId(1),
            },
        )
        .await;
    }
    for socket in [&mut first, &mut second] {
        assert!(matches!(
            frame(socket).await,
            Frame::Event(Event::Catalogue {
                request: RequestId(1),
                result: Ok(_)
            })
        ));
        assert!(matches!(frame(socket).await, Frame::Finished(RequestId(1))));
    }
    first.send(Message::Text("not JSON".into())).await.unwrap();
    let Message::Close(Some(close)) = first.next().await.unwrap().unwrap() else {
        panic!("expected close");
    };
    assert_eq!(close.reason, "bad command");
    send(
        &mut second,
        &Command::LoadCatalogue {
            request: RequestId(2),
        },
    )
    .await;
    assert!(matches!(
        &finish(&mut second, RequestId(2)).await[..],
        [Event::Catalogue { result: Ok(_), .. }]
    ));
    for request in 100..164 {
        send(
            &mut second,
            &Command::LoadCatalogue {
                request: RequestId(request),
            },
        )
        .await;
    }
    let mut catalogues = std::collections::HashSet::new();
    let mut completions = std::collections::HashSet::new();
    while completions.len() < 64 {
        match frame(&mut second).await {
            Frame::Event(Event::Catalogue {
                request,
                result: Ok(_),
            }) => {
                assert!(!completions.contains(&request));
                assert!(catalogues.insert(request));
            }
            Frame::Finished(request) => {
                assert!(catalogues.contains(&request));
                assert!(completions.insert(request));
            }
            other => panic!("unexpected frame: {other:?}"),
        }
    }
    let choice = choices::ModelSettings {
        model: "gpt-6-astra".into(),
        thinking: "high".into(),
        ..Default::default()
    };
    send(
        &mut second,
        &Command::SaveModelChoice {
            request: RequestId(3),
            work: choices::WorkKind::Ask,
            choice: choice.clone(),
        },
    )
    .await;
    assert!(matches!(
        &finish(&mut second, RequestId(3)).await[..],
        [Event::ModelChoiceSaved { result: Ok(()), .. }]
    ));
    let client = server.client();
    let cookie = server.login(&client).await;
    let facts: StartupFacts = client
        .get(server.url("/api/facts"))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(facts.fixture.as_deref(), Some("black-scholes"));
    assert_eq!(facts.model_choices.of(choices::WorkKind::Ask), &choice);
}

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

    let ingest = IngestRequest {
        pdf: UploadId::default(),
        file_name: "sample.pdf".into(),
        media: "Sample".into(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 1,
            name: "Sample".into(),
        },
        tags: vec![],
    };
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
    // An ingest cancellation is cooperative; its slot remains held until the handler ends.
    let facts = running(&server, true).await;
    assert_eq!(facts.running[0].kind, contract::JobKind::Ingest);
    assert_eq!(facts.running[0].name, "sample.pdf");
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
        &finish(&mut third, RequestId(14)).await[..],
        [Event::IngestFinished { result: Err(_), .. }]
    ));
    running(&server, false).await;
    send(&mut third, &dataset(15)).await;
    let events = finish(&mut third, RequestId(15)).await;
    assert!(matches!(
        events.last(),
        Some(Event::DatasetFinished { result: Ok(_), .. })
    ));
}

async fn handshake(
    server: &TestServer,
    headers: &[(&str, &str)],
) -> Result<super::support::Socket, tokio_tungstenite::tungstenite::Error> {
    let mut request = format!("ws://{}/api/ws", server.address)
        .into_client_request()
        .unwrap();
    for (name, value) in headers {
        let name = name.parse::<axum::http::HeaderName>().unwrap();
        if name == axum::http::header::HOST {
            request.headers_mut().insert(name, value.parse().unwrap());
        } else {
            request.headers_mut().append(name, value.parse().unwrap());
        }
    }
    tokio_tungstenite::connect_async(request)
        .await
        .map(|(socket, _)| socket)
}
