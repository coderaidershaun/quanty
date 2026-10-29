//! Exported picture URLs stay inside content, including when a link points elsewhere.
use backend::{Handler, Reply, fake::Fake};
use contract::{Command, DocumentView, Event, ExportFile, Failure, Loadable, RequestId};
use export::{Block, Document, Format};
use gui::state::Contents;

use crate::support::served;

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

#[tokio::test]
async fn a_library_tab_export_writes_that_tabs_content() {
    let home = tempfile::tempdir().unwrap();
    let samples = home.path().join("samples");
    super::support::copy_folder(gui::testkit::samples_folder().parent().unwrap(), &samples);
    let fake = Fake::scene("idle", home.path()).unwrap().instant();
    let request = RequestId(1);

    let events = served(&fake, Command::LoadCatalogue { request }).await;
    let Some(Event::Catalogue {
        result: Ok(catalogue),
        ..
    }) = events.into_iter().next()
    else {
        panic!("expected the catalogue first");
    };
    let document = catalogue
        .documents()
        .find(|document| document.title.contains("Black Scholes"))
        .expect("the samples hold the Black Scholes chapter");
    let command = Command::LoadChapterContents {
        request,
        doc: document.id,
        folder: document.folder.clone(),
    };
    let sent = served(&fake, command).await;
    let [
        Event::ChapterContents {
            result: Ok(chapter),
            ..
        },
        ..,
    ] = sent.as_slice()
    else {
        panic!("expected the contents of the chapter first");
    };
    let listing = chapter
        .of_kind(contract::PieceKind::Code)
        .next()
        .expect("the chapter has a code listing")
        .piece
        .text
        .clone();
    let contents = Contents {
        doc: Some(document.id),
        items: Loadable::Ready(chapter.clone()),
        ..Contents::default()
    };

    let asked = contents
        .export_request(&catalogue, DocumentView::Code, Format::Html)
        .expect("a chapter with code can export its code");
    let (reply, events, _stop) = Reply::collecting();
    fake.serve(
        Command::Export {
            request,
            file: ExportFile {
                format: asked.format,
                document: asked.document,
                file_name: asked.file_name,
            },
        },
        reply,
    )
    .await;
    let Some(Event::Exported {
        result: Ok(download),
        ..
    }) = events.try_iter().next()
    else {
        panic!("expected the export to be written");
    };
    let id = download
        .url
        .strip_prefix("/api/download/")
        .unwrap()
        .split('/')
        .next()
        .unwrap();
    let written = std::fs::read_to_string(
        home.path()
            .join("data/exports")
            .join(id)
            .join(&download.file_name),
    )
    .unwrap();
    let first_line = listing.lines().next().unwrap().trim();
    assert!(
        written.contains(&first_line.replace('&', "&amp;").replace('<', "&lt;")),
        "the file holds the listing, which starts `{first_line}`"
    );
}
