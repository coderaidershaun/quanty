//! "Edit tags" changes only the own tags of a document, and a refused save shows its hint.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use gui::contract::{Command, DocumentTagsEdit};
use gui::testkit;

use super::{Ingests, REFUSED_HINT, documents_of, open_switched};
use crate::flows::recording::Seen;
use crate::flows::{Window, click, field, has, is_enabled, press, retype, says};

fn sent_edits(seen: &Seen) -> Vec<DocumentTagsEdit> {
    seen.all()
        .into_iter()
        .filter_map(|command| match command {
            Command::SetDocumentTags { edit, .. } => Some(edit),
            _ => None,
        })
        .collect()
}

fn a_refused_save_shows_its_hint_and_keeps_what_was_typed(
    harness: &mut Window,
    seen: &Seen,
    refuses_saves: &AtomicBool,
) {
    let before = documents_of(harness).remove(0);
    click(harness, Role::Button, "Edit tags");
    assert!(
        !has(harness, Role::TextInput, "Authors"),
        "the authors belong to the media"
    );
    assert_eq!(field(harness, "Tags").as_deref(), Some("point-processes"));
    assert!(
        !is_enabled(harness, Role::Button, "Save"),
        "nothing was changed, so there is nothing to save"
    );
    testkit::save_png(harness, "app-library-edit");

    retype(harness, "Tags", "Volatility, greeks");
    refuses_saves.store(true, Ordering::SeqCst);
    click(harness, Role::Button, "Save");

    let wanted = DocumentTagsEdit {
        doc: before.id,
        add: vec!["volatility".to_owned(), "greeks".to_owned()],
        remove: vec!["point-processes".to_owned()],
    };
    assert_eq!(sent_edits(seen), [wanted]);
    assert!(says(harness, REFUSED_HINT), "the refusal says what to do");
    assert_eq!(
        field(harness, "Tags").as_deref(),
        Some("Volatility, greeks")
    );
    assert_eq!(
        documents_of(harness).remove(0),
        before,
        "nothing was stored"
    );
    testkit::save_png(harness, "app-library-refused");
}

fn the_same_save_goes_through_and_the_new_tags_show_everywhere(
    harness: &mut Window,
    seen: &Seen,
    refuses_saves: &AtomicBool,
) {
    refuses_saves.store(false, Ordering::SeqCst);
    let loads = seen.count(|command| matches!(command, Command::LoadCatalogue { .. }));
    click(harness, Role::Button, "Save");
    assert_eq!(sent_edits(seen).len(), 2);
    assert_eq!(
        seen.count(|command| matches!(command, Command::LoadCatalogue { .. })),
        loads + 1,
        "the catalogue is loaded again after the save"
    );
    assert!(!has(harness, Role::TextInput, "Tags"), "the form is closed");
    let first = documents_of(harness).remove(0);
    assert_eq!(first.tags, ["greeks", "volatility"]);
    assert_eq!(
        first.media_tags,
        ["hawkes"],
        "the tags of the media are not the document's to change"
    );
    for tag in ["greeks", "volatility"] {
        assert!(has(harness, Role::Label, tag), "the tag `{tag}` is listed");
    }
    assert!(
        harness
            .query_all_by_label("point-processes")
            .next()
            .is_none(),
        "the tag `point-processes` was taken off"
    );
    testkit::save_png(harness, "app-library-relabelled");

    click(harness, Role::Tab, "Ask");
    click(harness, Role::ComboBox, "Tags");
    assert!(has(harness, Role::CheckBox, "greeks"));
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
}

#[test]
fn a_document_s_own_tags_are_edited_and_a_refusal_shows_its_hint() {
    let refuses_saves = Arc::new(AtomicBool::new(false));
    let (mut harness, seen) = open_switched("black-scholes", &refuses_saves, Ingests::Run);
    click(&mut harness, Role::Tab, "Library");
    a_refused_save_shows_its_hint_and_keeps_what_was_typed(&mut harness, &seen, &refuses_saves);
    the_same_save_goes_through_and_the_new_tags_show_everywhere(
        &mut harness,
        &seen,
        &refuses_saves,
    );
}
