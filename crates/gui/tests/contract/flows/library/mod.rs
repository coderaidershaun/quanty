//! The Library tab lists the stored media and their documents, copies the id of a document, opens
//! it to read, changes its own tags and the labels of a media, and says what no sample shows.

mod catalogue;
mod document_tags;
mod media;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{Category, Command, Document, Failure, FailureKind, Intent, MediaEdit};
use gui::testkit;

use crate::flows::recording::{self, Seen};
use crate::flows::{Window, is_enabled, node, says, shared};

const REFUSED_HINT: &str = "Start Qdrant at http://localhost:6334, then try again.";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ingests {
    Run,
    NeverEnd,
}

/// Wraps the fake so that a test can make it refuse a save of tags or of the labels of a media,
/// and make every ingest run for ever.
struct Switched<H> {
    inner: H,
    refuses_saves: Arc<AtomicBool>,
    ingests: Ingests,
}

impl<H: Handler> Handler for Switched<H> {
    async fn serve(&self, command: Command, reply: Reply) {
        match command {
            Command::SetDocumentTags { .. } | Command::EditMedia { .. }
                if self.refuses_saves.load(Ordering::SeqCst) =>
            {
                let failure = Failure::new(FailureKind::QdrantDown, "the test refuses the save")
                    .with_hint(REFUSED_HINT);
                for event in command.failed(&failure) {
                    reply.send(event);
                }
            }
            Command::Ingest { .. } if self.ingests == Ingests::NeverEnd => {}
            other => self.inner.serve(other, reply).await,
        }
    }
}

fn open_switched(scene: &str, refuses_saves: &Arc<AtomicBool>, ingests: Ingests) -> (Window, Seen) {
    let refuses_saves = Arc::clone(refuses_saves);
    let (mut harness, seen) =
        recording::open_with(scene, DEFAULT_WINDOW, move |fake: Fake| Switched {
            inner: fake,
            refuses_saves,
            ingests,
        });
    testkit::settle(&mut harness);
    (harness, seen)
}

fn documents_of(harness: &Window) -> Vec<Document> {
    let catalogue = shared(harness).library.catalogue.ready();
    let catalogue = catalogue.expect("the catalogue is loaded");
    catalogue.documents().cloned().collect()
}

fn sent_media_edits(seen: &Seen) -> Vec<MediaEdit> {
    seen.all()
        .into_iter()
        .filter_map(|command| match command {
            Command::EditMedia { edit, .. } => Some(edit),
            _ => None,
        })
        .collect()
}

#[test]
fn nothing_can_be_edited_while_an_ingest_runs() {
    let (mut harness, seen) = open_switched("ingest-ready", &Arc::default(), Ingests::NeverEnd);
    // The ingest never ends, so the app is never idle again: `click` and `settle` would wait
    // for ever.
    node(&harness, Role::Button, "Start ingest").click();
    harness.run_ok();
    assert!(shared(&harness).ingest.is_running());
    node(&harness, Role::Tab, "Library").click();
    harness.run_ok();
    let buttons: Vec<_> = harness
        .query_all_by_role_and_label(Role::Button, "Edit tags")
        .collect();
    assert!(!buttons.is_empty());
    for button in buttons {
        assert!(button.accesskit_node().is_disabled());
    }
    let catalogue = shared(&harness).library.catalogue.ready().cloned();
    let catalogue = catalogue.expect("the catalogue is loaded");
    let titles: Vec<&str> = catalogue
        .media
        .iter()
        .filter_map(|media| media.title.as_deref())
        .collect();
    assert!(!titles.is_empty());
    for title in titles {
        let pencil = format!("Edit {title}");
        assert!(
            !is_enabled(&harness, Role::Button, &pencil),
            "the pencil of `{title}` is off while an ingest runs"
        );
    }
    assert!(says(&harness, "An ingest is running"));
    testkit::save_png(&mut harness, "app-library-ingest-running");

    let edit = MediaEdit {
        title: "Quanty Sample Notes".to_owned(),
        category: Category::Paper,
        ..MediaEdit::default()
    };
    harness.state_mut().push(Intent::EditMedia(edit));
    harness.run_ok();
    assert!(
        sent_media_edits(&seen).is_empty(),
        "an edit of a media is not sent while an ingest runs"
    );
}
