//! The Library tab lists the stored media on a shelf and shows the page of the open media or
//! document, finds a media or a chapter, is opened from the Source panel, copies the id of a
//! document, opens it to read, shows the contents and the insights of a chapter, changes its own
//! tags and the labels of a media, deletes a document or a media after asking, and says what no
//! sample shows.

mod catalogue;
mod contents;
mod delete;
mod delete_on_its_way;
mod document_tags;
mod insights;
mod media;
mod navigation;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use backend::fake::Fake;
use backend::{Handler, Reply};
use contract::{Category, Command, Document, Failure, FailureKind, Intent, MediaEdit};
use eframe::egui;
use eframe::egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::NodeT as _;
use gui::app::layout::DEFAULT_WINDOW;
use gui::testkit;

use crate::flows::recording::{self, Seen};
use crate::flows::{Window, click, has, hover_on, is_enabled, node, panels, says, shared};

const REFUSED_HINT: &str = "Start Qdrant at http://localhost:6334, then try again.";

/// The titles of the sample library that the scenes list.
const HAWKES: &str = "Hawkes Processes in Finance";
const PAPER: &str = "Self-Exciting Order Flow";
const BOOK: &str = "Option Volatility and Pricing";
const SAMPLE_PAGES: &str = "Chapter 1 · Sample Pages";
const NOTES: &str = "Quanty Sample Notes";
const BLACK_SCHOLES: &str = "Chapter 2 · Black Scholes In Depth";

/// The frame of the page cuts off the last points of the page, so a control counts as in view only
/// when it is this far inside the page.
const VIEW_INSET: f32 = 32.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ingests {
    Run,
    NeverEnd,
}

/// Wraps the fake so that a test can make it refuse a save of tags or of the labels of a media and
/// a delete of a document or of a media, and make every ingest run for ever.
struct Switched<H> {
    inner: H,
    refuses_changes: Arc<AtomicBool>,
    ingests: Ingests,
}

impl<H: Handler> Handler for Switched<H> {
    async fn serve(&self, command: Command, reply: Reply) {
        match command {
            Command::SetDocumentTags { .. }
            | Command::EditMedia { .. }
            | Command::DeleteDocument { .. }
            | Command::DeleteMedia { .. }
                if self.refuses_changes.load(Ordering::SeqCst) =>
            {
                let failure = Failure::new(FailureKind::QdrantDown, "the test refuses the change")
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

fn open_switched(
    scene: &str,
    refuses_changes: &Arc<AtomicBool>,
    ingests: Ingests,
) -> (Window, Seen) {
    let refuses_changes = Arc::clone(refuses_changes);
    let (mut harness, seen) =
        recording::open_with(scene, DEFAULT_WINDOW, move |fake: Fake| Switched {
            inner: fake,
            refuses_changes,
            ingests,
        });
    testkit::settle(&mut harness);
    (harness, seen)
}

fn view() -> egui::Rect {
    panels(DEFAULT_WINDOW).page.shrink(VIEW_INSET)
}

/// Presses the row of a media on the shelf, so that its page shows and its documents unfold.
fn open_media(harness: &mut Window, title: &str) {
    click(harness, Role::Button, title);
}

/// Opens the media, then presses the row of one of its documents.
fn open_document(harness: &mut Window, media: &str, label: &str) {
    open_media(harness, media);
    click(harness, Role::Button, label);
}

/// A row of the shelf is chosen as a toggle is: the open item is the row that is on.
fn is_open_row(harness: &Window, label: &str) -> bool {
    node(harness, Role::Button, label)
        .accesskit_node()
        .toggled()
        == Some(Toggled::True)
}

/// As a click on a row, for an app that is never at rest: a change on its way or an ingest that
/// never ends, where `click` would wait for ever.
fn open_row_while_busy(harness: &mut Window, label: &str) {
    node(harness, Role::Button, label).click();
    harness.run_ok();
}

/// Visits every media page and every document page of an app that is never at rest, and finds
/// the pencil, the tags button and the Delete of each off.
fn every_change_is_off_on_every_page(harness: &mut Window) {
    let catalogue = shared(harness).library.catalogue.ready().cloned();
    let catalogue = catalogue.expect("the catalogue is loaded");
    let mut pages = 0;
    for media in &catalogue.media {
        let title = media.title.as_deref().expect("a sample media has a title");
        open_row_while_busy(harness, title);
        for button in [format!("Edit {title}"), format!("Delete media {title}")] {
            assert!(
                !is_enabled(harness, Role::Button, &button),
                "`{button}` is off"
            );
        }
        for document in &media.documents {
            let label = document.name().label();
            open_row_while_busy(harness, &label);
            // The button of the tags is not there while the form of the tags is open.
            if !has(harness, Role::TextInput, "Tags") {
                assert!(
                    !is_enabled(harness, Role::Button, "Edit tags"),
                    "`Edit tags` is off"
                );
            }
            let delete = format!("Delete document {label}");
            assert!(
                !is_enabled(harness, Role::Button, &delete),
                "`{delete}` is off"
            );
            pages += 1;
        }
    }
    assert!(pages > 1, "the documents were visited");
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
    every_change_is_off_on_every_page(&mut harness);
    open_row_while_busy(&mut harness, HAWKES);
    open_row_while_busy(&mut harness, PAPER);
    hover_on(
        &mut harness,
        Role::Button,
        &format!("Delete document {PAPER}"),
    );
    assert!(says(
        &harness,
        "Nothing can be deleted while an ingest runs."
    ));
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

    let doc = documents_of(&harness)[0].id;
    harness.state_mut().push(Intent::DeleteDocument(doc));
    harness
        .state_mut()
        .push(Intent::DeleteMedia("Quanty Sample Notes".to_owned()));
    harness.run_ok();
    let deletes = seen.count(|command| {
        matches!(
            command,
            Command::DeleteDocument { .. } | Command::DeleteMedia { .. }
        )
    });
    assert_eq!(deletes, 0, "no delete is sent while an ingest runs");
}
