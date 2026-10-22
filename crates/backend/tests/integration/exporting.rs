//! Exported picture URLs stay inside content, including when a link points elsewhere.
use backend::{Handler, Reply, fake::Fake};
use contract::{Command, Event, ExportFile, Failure, RequestId};
use export::{Block, Document, Format};

#[tokio::test]
async fn export_downloads_resolve_content_pictures_and_refuse_escapes() {
    let home = tempfile::tempdir().unwrap();
    let samples = home.path().join("samples");
    super::support::copy_folder(gui::testkit::samples_folder().parent().unwrap(), &samples);
    let fake = Fake::scene("idle", home.path()).unwrap().instant();
    let fake = &fake;
    let exported = |picture: &'static str| async move {
        let (reply, events, _stop) = Reply::collecting();
        let document = Document {
            blocks: vec![Block::Figure {
                picture: Some(picture.into()),
                description: "fallback description".into(),
                caption: String::new(),
                cite: None,
            }],
            ..Default::default()
        };
        fake.serve(
            Command::Export {
                request: RequestId(1),
                file: ExportFile {
                    format: Format::Html,
                    document,
                    file_name: "answer.html".into(),
                },
            },
            reply,
        )
        .await;
        match events.try_recv().unwrap() {
            Event::Exported { result, .. } => result,
            event => panic!("unexpected export reply: {event:?}"),
        }
    };
    let download =
        exported("/content/option-volatility-and-pricing/chapter-1/page-num-5/01-figure.png")
            .await
            .unwrap();
    let id = download
        .url
        .strip_prefix("/api/download/")
        .unwrap()
        .split('/')
        .next()
        .unwrap();
    let text = std::fs::read_to_string(
        home.path()
            .join("data/exports")
            .join(id)
            .join(&download.file_name),
    )
    .unwrap();
    assert!(text.contains("data:image/png;base64,"));
    let missing = exported("/content/missing.png").await.unwrap();
    let id = missing
        .url
        .strip_prefix("/api/download/")
        .unwrap()
        .split('/')
        .next()
        .unwrap();
    let text = std::fs::read_to_string(
        home.path()
            .join("data/exports")
            .join(id)
            .join(&missing.file_name),
    )
    .unwrap();
    assert!(text.contains("fallback description"));
    for path in [
        "/content/../outside.png",
        "/content/%2e%2e/outside.png",
        "/etc/passwd",
    ] {
        let failure: Failure = exported(path).await.unwrap_err();
        assert!(failure.detail.contains("content"));
    }
    #[cfg(unix)]
    {
        let outside = home.path().join("outside.png");
        std::fs::copy(
            samples
                .join("content/option-volatility-and-pricing/chapter-1/page-num-5/01-figure.png"),
            &outside,
        )
        .unwrap();
        std::os::unix::fs::symlink(&outside, samples.join("content/escape.png")).unwrap();
        assert!(exported("/content/escape.png").await.is_err());
        std::os::unix::fs::symlink(home.path().join("absent"), samples.join("content/dangling"))
            .unwrap();
        assert!(exported("/content/dangling/missing.png").await.is_err());
    }
}
