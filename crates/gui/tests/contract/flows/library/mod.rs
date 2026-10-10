//! The Library tab lists the stored media and their documents, copies the id of a document, opens
//! it to read, changes its own tags and the labels of a media, deletes a document or a media after
//! asking, and says what no sample shows.

mod catalogue;
mod delete;
mod delete_on_its_way;
mod document_tags;
mod media;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::Node;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{Category, Command, Document, Failure, FailureKind, Intent, MediaEdit};
use gui::testkit;

use crate::flows::recording::{self, Seen};
use crate::flows::{Window, click, is_enabled, node, panels, says, shared};

const REFUSED_HINT: &str = "Start Qdrant at http://localhost:6334, then try again.";

/// The frame of the page and the edge of its list cut off the last points of the page, so a
/// control counts as in view only when it is this far inside the page.
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

/// egui keeps a node for a control that the list cuts off, and a click lands in the middle of the
/// node, so the click would miss it. The wheel is turned until the whole control is in view. It
/// only turns down, so cards are pressed from the top of the page to the bottom.
fn bring_into_view(harness: &mut Window, role: Role, name: &str) {
    let view = view();
    for _ in 0..40 {
        if view.contains_rect(node(harness, role, name).rect()) {
            return;
        }
        harness.hover_at(view.center());
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -240.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_ok();
    }
    panic!("`{name}` never came wholly into view");
}

fn click_on_the_card(harness: &mut Window, role: Role, name: &str) {
    bring_into_view(harness, role, name);
    click(harness, role, name);
}

/// Every button of a card that deletes: `Delete document …` and `Delete media …`.
fn delete_buttons(harness: &Window) -> Vec<Node<'_>> {
    harness
        .query_all_by_role(Role::Button)
        .filter(|button| {
            let name = button.accesskit_node().label();
            name.is_some_and(|name| name.starts_with("Delete "))
        })
        .collect()
}

/// The text of a button that is off shows once the pointer has rested on it, and not for the
/// button that was clicked last, so a test hovers another one. The app is never at rest when this
/// is used, so `run_ok` runs the four frames that the rest takes.
fn hover_on(harness: &mut Window, role: Role, name: &str) {
    node(harness, role, name).hover();
    harness.run_ok();
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
    let deletes = delete_buttons(&harness);
    assert!(!deletes.is_empty());
    for button in &deletes {
        assert!(button.accesskit_node().is_disabled(), "{button:?} is off");
    }
    let first_delete = deletes[0].accesskit_node().label().unwrap_or_default();
    hover_on(&mut harness, Role::Button, &first_delete);
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
