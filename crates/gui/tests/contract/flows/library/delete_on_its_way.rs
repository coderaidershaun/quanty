//! While a delete or a save is on its way, its card shows it, and no other change of the library
//! starts: every button that changes the library is off and says why, and what is pushed anyway
//! sends nothing.

use eframe::egui::accesskit::{Action, Role};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{Category, Command, DocId, DocumentTagsEdit, Intent, MediaEdit, NewMedia};
use gui::state::Busy;
use gui::testkit;

use super::{click_on_the_card, delete_buttons, documents_of, hover_on};
use crate::flows::recording::{self, Seen};
use crate::flows::{Window, click, has, is_enabled, node, says, shared, type_into};

const UPDATING: &str = "Nothing can be deleted while the library is being updated.";
const NOTES: &str = "Quanty Sample Notes";
const PAPER: &str = "Self-Exciting Order Flow";

/// Swallows the changes that this file sends, so each stays on its way for ever, and hands every
/// other command on.
struct Held<H>(H);

impl<H: Handler> Handler for Held<H> {
    async fn serve(&self, command: Command, reply: Reply) {
        match command {
            Command::SetDocumentTags { .. }
            | Command::DeleteDocument { .. }
            | Command::DeleteMedia { .. } => {}
            other => self.0.serve(other, reply).await,
        }
    }
}

/// The scene opens on the Ingest tab with a checked chapter, so the Library tab is opened first.
fn open_held() -> (Window, Seen) {
    let (mut harness, seen) =
        recording::open_with("ingest-ready", DEFAULT_WINDOW, |fake: Fake| Held(fake));
    testkit::settle(&mut harness);
    click(&mut harness, Role::Tab, "Library");
    (harness, seen)
}

/// The app is never at rest after the change is sent, so `settle` would wait for ever.
fn send_the_change_and_run(harness: &mut Window, role: Role, name: &str) {
    node(harness, role, name).click();
    harness.run_ok();
}

fn the_other_changes_are_off(harness: &Window) {
    for button in harness.query_all_by_role_and_label(Role::Button, "Edit tags") {
        assert!(button.accesskit_node().is_disabled(), "`Edit tags` is off");
    }
    let catalogue = shared(harness).library.catalogue.ready().cloned();
    let catalogue = catalogue.expect("the catalogue is loaded");
    for title in catalogue
        .media
        .iter()
        .filter_map(|media| media.title.as_deref())
    {
        let pencil = format!("Edit {title}");
        assert!(
            !is_enabled(harness, Role::Button, &pencil),
            "the pencil of `{title}` is off"
        );
    }
    let deletes = delete_buttons(harness);
    assert!(deletes.len() > 1);
    for button in deletes {
        assert!(button.accesskit_node().is_disabled(), "{button:?} is off");
    }
}

/// A button that loads senses only a hover, so it has no click action. A button that is only off
/// keeps it.
fn only_the_busy_button_shows_that_it_is_busy(harness: &Window, busy: &str, other: &str) {
    let supports_click = |name: &str| {
        let button = node(harness, Role::Button, name);
        button
            .accesskit_node()
            .data()
            .supports_action(Action::Click)
    };
    assert!(!supports_click(busy), "`{busy}` shows that it is busy");
    assert!(supports_click(other), "`{other}` is only off");
}

fn the_off_buttons_say_why(harness: &mut Window, other: &str) {
    hover_on(harness, Role::Button, other);
    assert!(says(harness, UPDATING));
}

/// A start sends an ingest that goes on after its conversion with no lock, so it waits for the
/// delete as the delete waits for it.
fn the_ingest_tab_will_not_start_while_a_delete_is_on_its_way(harness: &mut Window) {
    node(harness, Role::Tab, "Ingest").click();
    harness.run_ok();
    assert!(!is_enabled(harness, Role::Button, "Start ingest"));
    hover_on(harness, Role::Button, "Start ingest");
    assert!(says(
        harness,
        "A delete is on its way. Start when it is done."
    ));
}

/// A change that is pushed anyway is dropped by the state, so the backend never gets it.
fn pushing_every_other_change_sends_nothing(harness: &mut Window, seen: &Seen, others: &[Intent]) {
    let before = seen.all();
    for intent in others {
        harness.state_mut().push(intent.clone());
    }
    harness.run_ok();
    assert_eq!(seen.all(), before);
}

fn every_change() -> Vec<Intent> {
    vec![
        Intent::SetDocumentTags(DocumentTagsEdit {
            doc: DocId::default(),
            add: vec!["tag".to_owned()],
            remove: Vec::new(),
        }),
        Intent::EditMedia(MediaEdit {
            title: NOTES.to_owned(),
            category: Category::Paper,
            ..MediaEdit::default()
        }),
        Intent::SaveMedia(NewMedia {
            title: "Dynamic Hedging".to_owned(),
            ..NewMedia::default()
        }),
        Intent::StartIngest,
    ]
}

fn a_document_delete_on_its_way_is_seen_on_its_card_and_holds_every_other_change() {
    let (mut harness, seen) = open_held();
    let paper = documents_of(&harness)
        .into_iter()
        .find(|document| document.title == PAPER)
        .expect("the paper is listed");
    let busy = format!("Delete document {PAPER}");
    let other = "Delete document Chapter 1 · Sample Pages";
    click_on_the_card(&mut harness, Role::Button, &busy);
    send_the_change_and_run(&mut harness, Role::Button, "Delete document");

    assert!(matches!(
        shared(&harness).library.busy.get(&paper.id),
        Some(Busy::Deleting(_))
    ));
    only_the_busy_button_shows_that_it_is_busy(&harness, &busy, other);
    the_other_changes_are_off(&harness);
    the_off_buttons_say_why(&mut harness, other);
    let mut others = every_change();
    others.push(Intent::DeleteDocument(documents_of(&harness)[1].id));
    others.push(Intent::DeleteMedia(NOTES.to_owned()));
    pushing_every_other_change_sends_nothing(&mut harness, &seen, &others);
    testkit::save_png(&mut harness, "app-library-deleting");
    the_ingest_tab_will_not_start_while_a_delete_is_on_its_way(&mut harness);
}

fn a_media_delete_on_its_way_is_seen_on_its_card_and_holds_every_other_change() {
    let (mut harness, seen) = open_held();
    let busy = format!("Delete media {NOTES}");
    let other = "Delete media Hawkes Processes in Finance";
    click_on_the_card(&mut harness, Role::Button, &busy);
    send_the_change_and_run(&mut harness, Role::Button, "Delete media");

    assert!(shared(&harness).library.media_delete.is_deleting());
    only_the_busy_button_shows_that_it_is_busy(&harness, &busy, other);
    the_other_changes_are_off(&harness);
    the_off_buttons_say_why(&mut harness, other);
    let mut others = every_change();
    others.push(Intent::DeleteDocument(documents_of(&harness)[0].id));
    pushing_every_other_change_sends_nothing(&mut harness, &seen, &others);
}

fn a_save_of_tags_on_its_way_turns_every_delete_off() {
    let (mut harness, _seen) = open_held();
    click(&mut harness, Role::Button, "Edit tags");
    type_into(&mut harness, "Tags", "Volatility");
    send_the_change_and_run(&mut harness, Role::Button, "Save");

    assert!(matches!(
        shared(&harness).library.busy.values().next(),
        Some(Busy::Saving(_))
    ));
    let deletes = delete_buttons(&harness);
    assert!(!deletes.is_empty());
    for button in deletes {
        assert!(button.accesskit_node().is_disabled(), "{button:?} is off");
    }
    hover_on(
        &mut harness,
        Role::Button,
        "Delete document Chapter 1 · Sample Pages",
    );
    assert!(says(&harness, UPDATING));
    assert!(has(&harness, Role::Button, "Save"));
}

#[test]
fn a_change_on_its_way_shows_on_its_card_and_holds_every_other_change() {
    a_document_delete_on_its_way_is_seen_on_its_card_and_holds_every_other_change();
    a_media_delete_on_its_way_is_seen_on_its_card_and_holds_every_other_change();
    a_save_of_tags_on_its_way_turns_every_delete_off();
}
