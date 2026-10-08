//! The pencil on the card of a media opens a form that changes its category, its authors and its
//! tags, and a refused save shows its hint.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;
use eframe::egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::contract::{Category, Command, Media, MediaEdit};
use gui::testkit;

use super::{Ingests, REFUSED_HINT, open_switched, sent_media_edits};
use crate::flows::recording::Seen;
use crate::flows::{Window, click, field, has, is_enabled, node, panels, retype, shared};

/// The third card of the scene: a book by one author, with no media tags and two documents. Its
/// form opens near the bottom of the page.
const MEDIA: &str = "Quanty Sample Notes";
const PENCIL: &str = "Edit Quanty Sample Notes";

/// The frame of the page and the edge of its list cut off the last points of the page, so a
/// control counts as in view only when it is this far inside the page.
const VIEW_INSET: f32 = 32.0;

/// A chip tells a screen reader that it is chosen as a toggle, not as a selection.
fn is_toggled(harness: &Window, role: Role, name: &str) -> bool {
    node(harness, role, name).accesskit_node().toggled() == Some(Toggled::True)
}

fn view() -> egui::Rect {
    panels(DEFAULT_WINDOW).page.shrink(VIEW_INSET)
}

/// egui keeps a node for a control that the list cuts off, and a click lands in the middle of the
/// node, so the click would miss it. The wheel is turned until the whole control is in view.
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

fn retype_on_the_card(harness: &mut Window, name: &str, text: &str) {
    bring_into_view(harness, Role::TextInput, name);
    retype(harness, name, text);
}

fn the_media(harness: &Window) -> Media {
    let catalogue = shared(harness).library.catalogue.ready();
    let catalogue = catalogue.expect("the catalogue is loaded");
    let media = catalogue.media_titled(MEDIA);
    media.cloned().expect("the media keeps its title")
}

fn the_pencil_opens_the_form_with_the_labels_of_the_media(harness: &mut Window) {
    click_on_the_card(harness, Role::Button, PENCIL);
    assert_eq!(field(harness, "Authors").as_deref(), Some("Quanty Team"));
    assert_eq!(field(harness, "Tags").as_deref(), Some(""));
    assert!(
        is_toggled(harness, Role::Button, "Book"),
        "the category of the media is chosen"
    );
    let boxes: Vec<String> = harness
        .query_all_by_role(Role::TextInput)
        .map(|text_box| text_box.accesskit_node().label().unwrap_or_default())
        .collect();
    assert_eq!(
        boxes,
        ["Authors", "Tags"],
        "the title names the media, so no box can change it"
    );
    assert!(
        !is_enabled(harness, Role::Button, "Save media"),
        "nothing was changed, so there is nothing to save"
    );
    assert!(
        !has(harness, Role::Button, PENCIL),
        "the pencil is gone while its form is open"
    );
    testkit::save_png(harness, "app-library-edit-media");
}

fn cancel_closes_the_form_and_sends_nothing(harness: &mut Window, seen: &Seen) {
    click_on_the_card(harness, Role::Button, "Cancel");
    assert!(
        !has(harness, Role::TextInput, "Authors"),
        "the form is closed"
    );
    assert!(sent_media_edits(seen).is_empty());
}

fn a_refused_save_shows_its_hint_and_keeps_what_was_typed(
    harness: &mut Window,
    seen: &Seen,
    refuses_saves: &AtomicBool,
) {
    let before = the_media(harness);
    click_on_the_card(harness, Role::Button, PENCIL);
    click_on_the_card(harness, Role::Button, "Other");
    retype_on_the_card(harness, "Authors", "Ann Writer, Bo Writer");
    retype_on_the_card(harness, "Tags", "Teaching");
    refuses_saves.store(true, Ordering::SeqCst);
    click_on_the_card(harness, Role::Button, "Save media");

    let wanted = MediaEdit {
        title: MEDIA.to_owned(),
        category: Category::Other,
        authors: vec!["Ann Writer".to_owned(), "Bo Writer".to_owned()],
        tags: vec!["Teaching".to_owned()],
    };
    assert_eq!(sent_media_edits(seen), [wanted]);
    let refusal = harness.query_all_by_label_contains(REFUSED_HINT).next();
    let refusal = refusal.expect("the refusal says what to do");
    assert!(
        view().contains_rect(refusal.rect()),
        "the refusal at {:?} is brought into the view {:?}, so the person sees why the save failed",
        refusal.rect(),
        view()
    );
    assert_eq!(
        field(harness, "Authors").as_deref(),
        Some("Ann Writer, Bo Writer")
    );
    assert_eq!(the_media(harness), before, "nothing was stored");
    testkit::save_png(harness, "app-library-media-refused");
}

fn the_same_save_goes_through_and_the_card_shows_the_new_labels(
    harness: &mut Window,
    seen: &Seen,
    refuses_saves: &AtomicBool,
) {
    let before = the_media(harness);
    let shown_after = ["Other", "Ann Writer, Bo Writer", "teaching"];
    for label in shown_after {
        assert!(
            !has(harness, Role::Label, label),
            "`{label}` is not shown yet"
        );
    }
    refuses_saves.store(false, Ordering::SeqCst);
    let loads = seen.count(|command| matches!(command, Command::LoadCatalogue { .. }));
    click_on_the_card(harness, Role::Button, "Save media");

    assert_eq!(sent_media_edits(seen).len(), 2);
    assert_eq!(
        seen.count(|command| matches!(command, Command::LoadCatalogue { .. })),
        loads + 1,
        "the catalogue is loaded again after the save"
    );
    assert!(
        !has(harness, Role::TextInput, "Authors"),
        "the form is closed"
    );
    let after = the_media(harness);
    assert_eq!(after.category, Category::Other);
    assert_eq!(after.authors, ["Ann Writer", "Bo Writer"]);
    assert_eq!(after.tags, ["teaching"]);
    assert_eq!(after.documents.len(), 2);
    for (document, old) in after.documents.iter().zip(&before.documents) {
        assert_eq!(document.authors, ["Ann Writer", "Bo Writer"]);
        assert_eq!(document.media_tags, ["teaching"]);
        assert_eq!(
            document.tags, old.tags,
            "the own tags of a document are not the media's to change"
        );
    }
    for label in shown_after {
        assert!(has(harness, Role::Label, label), "`{label}` is on the card");
    }
    testkit::save_png(harness, "app-library-media-edited");
}

#[test]
fn a_media_is_edited_from_its_card_and_a_refusal_shows_its_hint() {
    let refuses_saves = Arc::new(AtomicBool::new(false));
    let (mut harness, seen) = open_switched("black-scholes", &refuses_saves, Ingests::Run);
    click(&mut harness, Role::Tab, "Library");
    the_pencil_opens_the_form_with_the_labels_of_the_media(&mut harness);
    cancel_closes_the_form_and_sends_nothing(&mut harness, &seen);
    a_refused_save_shows_its_hint_and_keeps_what_was_typed(&mut harness, &seen, &refuses_saves);
    the_same_save_goes_through_and_the_card_shows_the_new_labels(
        &mut harness,
        &seen,
        &refuses_saves,
    );
}
