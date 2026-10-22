//! A real export becomes one authenticated attachment and is consumed once.

use super::support::{configured, finish, send};
use contract::{Command, Event, ExportFile, RequestId};
use export::{Document, Format};

#[tokio::test]
async fn exports_download_once_with_a_contained_filename_and_old_folder_cleanup() {
    let server = configured(Some("secret"), |settings, _| {
        let old = settings.home.join("data/exports/old");
        std::fs::create_dir_all(&old).unwrap();
        let times = std::fs::FileTimes::new()
            .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 86400));
        std::fs::File::open(old).unwrap().set_times(times).unwrap();
    })
    .await;
    assert!(!server.home.path().join("data/exports/old").exists());
    let mut socket = server.socket().await;
    send(
        &mut socket,
        &Command::Export {
            request: RequestId(1),
            file: ExportFile {
                format: Format::Pdf,
                document: Document {
                    title: "Sample export".into(),
                    ..Default::default()
                },
                file_name: "sample.pdf".into(),
            },
        },
    )
    .await;
    let events = finish(&mut socket, RequestId(1)).await;
    let [
        Event::Exported {
            result: Ok(download),
            ..
        },
    ] = &events[..]
    else {
        panic!("{events:?}");
    };
    let client = server.client();
    assert_eq!(
        client
            .get(server.url(&download.url))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let cookie = server.login(&client).await;
    let response = client
        .get(server.url(&download.url))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("attachment; filename=\"sample.pdf\"")
    );
    assert!(response.bytes().await.unwrap().starts_with(b"%PDF-"));
    assert_eq!(
        client
            .get(server.url(&download.url))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let id = uuid::Uuid::new_v4();
    let folder = server.home.path().join("data/exports").join(id.to_string());
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(server.home.path().join("private.pdf"), b"%PDF-secret").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        server.home.path().join("private.pdf"),
        folder.join("link.pdf"),
    )
    .unwrap();
    for url in [
        format!("/api/download/{id}/%2E%2E%2Fprivate.pdf"),
        format!("/api/download/{id}/link.pdf"),
    ] {
        assert_eq!(
            client
                .get(server.url(&url))
                .header("cookie", &cookie)
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    assert!(server.home.path().join("private.pdf").exists());
}
