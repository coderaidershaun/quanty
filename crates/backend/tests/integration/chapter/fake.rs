//! Checks that the fake backend reads every committed sample as the live backend reads it: the
//! items with their saved metadata, the saved insights and every page.

use std::path::Path;

use backend::Handler;
use backend::fake::Fake;
use contract::{Catalogue, Command, Event, Failure, PageView, RequestId};

use super::contents::loaded;
use super::insights::insights_of;
use crate::support::{closed, copy_folder, document_of, served};

const REQUEST: RequestId = RequestId(7);

async fn catalogue_of(fake: &Fake) -> Catalogue {
    let events = served(fake, Command::LoadCatalogue { request: REQUEST }).await;
    match events.as_slice() {
        [
            Event::Catalogue {
                result: Ok(catalogue),
                ..
            },
            ..,
        ] => catalogue.clone(),
        other => panic!("expected the catalogue of the samples first: {other:#?}"),
    }
}

async fn page_of(backend: &impl Handler, command: Command) -> Result<PageView, Failure> {
    let events = served(backend, command).await;
    match events.into_iter().next() {
        Some(Event::Page { result, .. }) => result,
        other => panic!("expected the page first: {other:#?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_fake_backend_reads_every_sample_as_the_live_backend_does() {
    let home = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fake = Fake::scene("idle", home)
        .expect("the scene should start")
        .instant();
    let catalogue = catalogue_of(&fake).await;
    let cx = closed(&gui::testkit::samples_folder());

    let (mut chapters, mut pages, mut with_insights) = (0, 0, 0);
    // The paper of the samples has no folder, so neither reader has anything of it.
    let with_folders = catalogue
        .documents()
        .filter_map(|document| Some((document, document.folder.clone()?)));
    for (document, folder) in with_folders {
        let (faked, _concepts) = loaded(&fake, document.id, Some(&folder)).await;
        let (live, _concepts) = loaded(&cx, document.id, Some(&folder)).await;
        let chapter = ocr::read_chapter(&folder).unwrap();
        let saved = ocr::content::ChapterArtifacts::read(&folder)
            .unwrap()
            .unwrap();
        assert!(
            saved.matches_source(&chapter.artifact_source()),
            "expected {:?}, saved {:?}",
            chapter.artifact_source(),
            saved.source
        );
        assert!(
            live.as_ref()
                .unwrap()
                .items
                .iter()
                .all(|item| item.metadata.is_some())
        );
        assert_eq!(
            faked.expect("the fake should list the contents"),
            live.expect("the live reader should list the contents"),
            "{}",
            document.title
        );
        chapters += 1;

        let faked = insights_of(&fake, document.id, Some(folder.clone())).await;
        let live = insights_of(&cx, document_of(&folder), Some(folder.clone())).await;
        assert_eq!(faked, live, "{}", document.title);
        with_insights += usize::from(matches!(live, Ok(Some(_))));

        for page in 1..=document.pages.expect("a sample has a page count") {
            let command = Command::LoadPage {
                request: REQUEST,
                doc: document.id,
                page,
                folder: Some(folder.clone()),
            };
            let faked = page_of(&fake, command.clone()).await;
            let live = page_of(&cx, command).await;
            assert_eq!(
                faked.expect("the fake should show the page"),
                live.expect("the live reader should show the page"),
                "{} page {page}",
                document.title
            );
            pages += 1;
        }
    }
    assert_eq!(chapters, 3, "three chapters");
    assert_eq!(pages, 13, "three chapters of 7, 3 and 3 pages");
    assert!(with_insights > 0, "one sample chapter carries insights");

    the_fake_checks_saved_metadata_as_the_live_backend_does(home, &catalogue).await;
}

async fn the_fake_checks_saved_metadata_as_the_live_backend_does(
    home: &Path,
    catalogue: &Catalogue,
) {
    let cx = closed(&gui::testkit::samples_folder());
    let copied = tempfile::tempdir().unwrap();
    copy_folder(&home.join("../../samples"), &copied.path().join("samples"));
    let fake = Fake::scene("idle", copied.path()).unwrap().instant();
    let document = catalogue
        .documents()
        .find(|doc| doc.folder.is_some())
        .unwrap();
    let folder = copied.path().join("samples/content").join(
        document
            .folder
            .as_ref()
            .unwrap()
            .canonicalize()
            .unwrap()
            .strip_prefix(home.join("../../samples/content").canonicalize().unwrap())
            .unwrap(),
    );
    let mut saved = ocr::content::ChapterArtifacts::read(&folder)
        .unwrap()
        .unwrap();
    for (title, description, valid) in [
        (
            "__builtin_prefetch",
            "Uses std::vector<std::vector<int>>, __VA_ARGS__ and #include <vector>.",
            true,
        ),
        (
            "#include <vector>",
            "Uses int **p, *ptr, a[i](x), $100 and prices < 120 > 0.",
            true,
        ),
        ("A **bold** title", "Keeps source text.", false),
        ("Plain title", "Uses `wrapped` text.", false),
    ] {
        saved.entries[0].text.title = title.to_owned();
        saved.entries[0].text.description = description.to_owned();
        saved.write(&folder).unwrap();
        let (faked, _) = loaded(&fake, document.id, Some(&folder)).await;
        let (live, _) = loaded(&cx, document.id, Some(&folder)).await;
        let faked = faked.unwrap();
        assert_eq!(
            faked
                .items
                .iter()
                .map(|item| &item.metadata)
                .collect::<Vec<_>>(),
            live.unwrap()
                .items
                .iter()
                .map(|item| &item.metadata)
                .collect::<Vec<_>>()
        );
        assert_eq!(faked.items[0].metadata.is_some(), valid);
    }
}
