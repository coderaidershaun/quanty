//! Draws the bar in a test window and checks what a person sees and what it sends.

use std::cell::Cell;
use std::rc::Rc;

use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use super::{Local, show};
use crate::contract::{AskDraft, AskMode, Catalogue, FailureKind, Filters, Intent, Loadable};
use crate::state::Shared;
use crate::testkit::{self, Host, sample};
use crate::theme::space;

const WIDE: [f32; 2] = [1504.0, 100.0];

/// Two books, one author and the tags `options` and `volatility`.
fn library() -> Catalogue {
    let mut catalogue = sample::catalogue();
    let first = &mut catalogue.books[0].chapters[0];
    first.author = Some("Sheldon Natenberg".to_owned());
    first.tags = vec!["options".to_owned(), "volatility".to_owned()];
    catalogue
}

/// The state with this catalogue loaded, as the app has it after the load arrives.
fn shared_with(catalogue: Catalogue) -> Shared {
    let mut shared = Shared::default();
    shared.library.catalogue = Loadable::Ready(catalogue);
    shared.library.revision += 1;
    shared
}

fn bar(size: [f32; 2], shared: Shared) -> Harness<'static, Host> {
    let mut local = Local::default();
    testkit::panel(size, shared, move |ui, cx| show(ui, &mut local, cx))
}

/// The title of the sample book at this place in the catalogue.
fn book_title(place: usize) -> String {
    let title = sample::catalogue().books[place].title.clone();
    title.expect("each sample book has a title")
}

/// Opens a list and clicks one of its rows.
fn choose(harness: &mut Harness<'_, Host>, list: &str, role: Role, row: &str) {
    harness.get_by_label(list).click();
    harness.run();
    harness.get_by_role_and_label(role, row).click();
    harness.run();
}

#[test]
fn asking_sends_the_trimmed_question_with_its_mode_and_filters() {
    let book = book_title(0);
    let mut harness = bar([1504.0, 400.0], shared_with(library()));
    harness.get_by_label("Question").click();
    harness
        .get_by_label("Question")
        .type_text("  What is vega?  ");
    harness.run();
    choose(&mut harness, "Mode", Role::Button, "Results only");
    choose(&mut harness, "Books", Role::Button, &book);
    choose(&mut harness, "Authors", Role::Button, "Sheldon Natenberg");
    choose(&mut harness, "Tags", Role::CheckBox, "options");
    // The tags list stays open after a tick, so the second tag is ticked in the same list.
    harness
        .get_by_role_and_label(Role::CheckBox, "volatility")
        .click();
    harness.run();
    testkit::save_png(&mut harness, "ask-bar-tags-open");

    // A click outside closes the list.
    harness.get_by_label("Question").click();
    harness.run();
    testkit::save_png(&mut harness, "ask-bar-typing");
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Ask").click();
    harness.run();

    let asked = Intent::Ask(AskDraft {
        question: "What is vega?".to_owned(),
        mode: AskMode::ResultsOnly,
        filters: Filters {
            book: Some(book),
            author: Some("Sheldon Natenberg".to_owned()),
            tags: vec!["options".to_owned(), "volatility".to_owned()],
        },
    });
    assert_eq!(harness.state().intents, vec![asked.clone(), asked]);
}

#[test]
fn a_bar_that_cannot_ask_says_why_and_sends_nothing() {
    let mut harness = bar(WIDE, Shared::default());
    harness.get_by_label("Type a question to ask.");
    testkit::save_png(&mut harness, "ask-bar-idle");

    harness.get_by_label("Question").click();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(
        harness.state().intents.is_empty(),
        "Enter asked a blank question"
    );
    assert!(
        harness.get_by_label("Question").is_focused(),
        "a refused Enter must keep the caret in the box"
    );

    harness.get_by_role_and_label(Role::Button, "Ask").click();
    harness.run();
    assert!(harness.state().intents.is_empty(), "the faded Ask asked");
}

#[test]
fn a_running_ask_can_be_stopped_but_not_by_the_double_click_that_started_it() {
    let mut harness = bar(WIDE, testkit::asked("q"));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Stop").click();
    harness.run();
    assert_eq!(harness.state().intents, vec![Intent::CancelAsk]);
    testkit::save_png(&mut harness, "ask-bar-running");

    // Enter asks anew while an ask runs; the reducer stops the old one.
    harness.state_mut().intents.clear();
    harness.get_by_label("Question").click();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.get_by_label("Question").type_text("What is theta?");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    let asked = Intent::Ask(AskDraft {
        question: "What is theta?".to_owned(),
        ..AskDraft::default()
    });
    assert_eq!(harness.state().intents, vec![asked]);

    // The second click of a double click on Ask lands on Stop. The clock is frozen so that two
    // test clicks, which are 0.25 seconds apart, count as a double click.
    let mut harness = bar(WIDE, Shared::default());
    harness.get_by_label("Question").click();
    harness.get_by_label("Question").type_text("q");
    harness.run();
    harness.input_mut().time = Some(10.0);
    harness.get_by_role_and_label(Role::Button, "Ask").click();
    harness.run();
    harness.state_mut().apply_intents();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Stop").click();
    harness.run();
    assert!(
        harness.state().intents.is_empty(),
        "the second click of the double click stopped the ask"
    );
}

#[test]
fn an_ask_from_anywhere_loads_into_the_bar() {
    let sent = AskDraft {
        question: "How is theta defined?".to_owned(),
        mode: AskMode::ResultsOnly,
        filters: Filters {
            book: Some(book_title(0)),
            author: None,
            tags: vec!["options".to_owned()],
        },
    };
    let mut harness = bar(WIDE, Shared::default());
    harness.state_mut().intents.push(Intent::Ask(sent.clone()));
    harness.state_mut().apply_intents();
    harness.run();

    assert_eq!(
        harness.get_by_label("Question").value().as_deref(),
        Some("How is theta defined?")
    );
    assert_eq!(
        harness.get_by_label("Mode").value().as_deref(),
        Some("Results only")
    );
    harness.get_by_label("Question").click();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(harness.state().intents, vec![Intent::Ask(sent)]);
}

#[test]
fn the_focus_cue_selects_the_question_and_types_nothing() {
    let passes = Rc::new(Cell::new(0_u32));
    let counted = Rc::clone(&passes);
    let mut local = Local::default();
    let mut harness = testkit::panel(
        WIDE,
        testkit::asked("How is vega defined?"),
        move |ui, cx| {
            counted.set(counted.get() + 1);
            show(ui, &mut local, cx);
        },
    );
    harness.run();
    assert!(!harness.get_by_label("Question").is_focused());

    // A real "/" press is a key event, which the shell takes, and a text event in the same frame.
    harness.state_mut().shared.cues.focus_ask_bar += 1;
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("/".to_owned()));
    let before = passes.get();
    harness.step();
    let question = harness.get_by_label("Question");
    assert!(
        question.is_focused(),
        "the caret is in the box in one frame"
    );
    assert_eq!(question.value().as_deref(), Some("How is vega defined?"));
    assert_eq!(
        passes.get() - before,
        2,
        "the frame is drawn again at once, so the caret shows without waiting for a repaint"
    );

    harness.get_by_label("Question").type_text("What is theta?");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    let asked = Intent::Ask(AskDraft {
        question: "What is theta?".to_owned(),
        ..AskDraft::default()
    });
    assert_eq!(harness.state().intents, vec![asked]);
}

#[test]
fn a_filter_value_that_left_the_catalogue_is_cleared() {
    let kept = book_title(1);
    let mut harness = bar([1504.0, 400.0], shared_with(library()));
    harness.get_by_label("Question").click();
    harness.get_by_label("Question").type_text("What is vega?");
    harness.run();
    choose(&mut harness, "Books", Role::Button, &kept);
    choose(&mut harness, "Authors", Role::Button, "Sheldon Natenberg");
    choose(&mut harness, "Tags", Role::CheckBox, "options");
    harness
        .get_by_role_and_label(Role::CheckBox, "volatility")
        .click();
    harness.run();

    // The first book was deleted: its author and the tag `volatility` are gone with it. The
    // chosen book and the tag `options` are still in the library, so they stay.
    let mut remaining = sample::catalogue();
    remaining.books.remove(0);
    remaining.books[0].chapters[0].tags = vec!["options".to_owned()];
    reload_and_ask(&mut harness, remaining);
    // Then the chosen book was deleted too, and nothing is left to filter on.
    reload_and_ask(&mut harness, Catalogue { books: Vec::new() });

    let no_filter = AskDraft {
        question: "What is vega?".to_owned(),
        ..AskDraft::default()
    };
    let mut what_stayed = no_filter.clone();
    what_stayed.filters.book = Some(kept);
    what_stayed.filters.tags = vec!["options".to_owned()];
    assert_eq!(
        harness.state().intents,
        vec![Intent::Ask(what_stayed), Intent::Ask(no_filter)]
    );
}

/// Puts this catalogue in place of the one before, as a reload does, then asks with Enter.
fn reload_and_ask(harness: &mut Harness<'_, Host>, catalogue: Catalogue) {
    let library = &mut harness.state_mut().shared.library;
    library.catalogue = Loadable::Ready(catalogue);
    library.revision += 1;
    // The click puts the caret in the box, and closes a list that is open.
    harness.get_by_label("Question").click();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
}

#[test]
fn a_library_that_is_loading_empty_or_failed_says_so_and_offers_the_way_out() {
    let mut loading = Shared::default();
    loading.library.catalogue = Loadable::Loading;
    let mut harness = bar(WIDE, loading);
    harness.run();
    harness.get_by_label("Loading the library\u{2026}");
    harness.get_by_role_and_label(Role::ProgressIndicator, "Library is loading");
    for filter in ["Books", "Authors", "Tags"] {
        let is_faded = harness.get_by_label(filter).accesskit_node().is_disabled();
        assert!(is_faded, "{filter} is live with nothing to choose");
    }
    testkit::save_png(&mut harness, "ask-bar-library-loading");

    let mut harness = bar(WIDE, shared_with(Catalogue { books: Vec::new() }));
    harness.get_by_label("Your library is empty. Add a chapter with rag-ingest.");
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Ingest a chapter")
            .is_none(),
        "the Ingest page is not built, so no button may lead to it"
    );
    testkit::save_png(&mut harness, "ask-bar-library-empty");

    let mut failed = Shared::default();
    failed.library.catalogue = Loadable::Failed(sample::failure(FailureKind::Internal));
    let mut harness = bar(WIDE, failed);
    harness.get_by_label(&format!(
        "The library did not load. {}",
        FailureKind::Internal.hint()
    ));
    harness
        .get_by_role_and_label(Role::Button, "Try again")
        .click();
    harness.run();
    assert_eq!(harness.state().intents, vec![Intent::RefreshCatalogue]);
    testkit::save_png(&mut harness, "ask-bar-library-failed");
}

#[test]
fn a_library_that_did_not_load_shows_the_error_while_the_pointer_is_on_its_line() {
    let failure = sample::failure(FailureKind::QdrantDown);
    let line = format!("The library did not load. {}", failure.hint);
    let mut failed = Shared::default();
    failed.library.catalogue = Loadable::Failed(failure.clone());
    let mut harness = bar(WIDE, failed);
    harness.run();
    harness.get_by_label(&line).hover();
    harness.run_ok();
    assert!(
        harness
            .query_all_by_label_contains(&failure.detail)
            .next()
            .is_some(),
        "the error behind the hint is not shown"
    );
}

#[test]
fn the_bar_fits_its_rectangle_at_both_window_sizes() {
    let long_title = "Derivatives ".repeat(5).trim_end().to_owned();
    let long_hint = "Start Qdrant and try again. "
        .repeat(4)
        .trim_end()
        .to_owned();
    let note = format!("The library did not load. {long_hint}");
    for width in [1504.0, 1148.0] {
        // A running ask on a book, while the library has not loaded: the book is in no list.
        let mut shared = Shared::default();
        let draft = AskDraft {
            question: "How is the Black\u{2013}Scholes formula derived?".to_owned(),
            filters: Filters {
                book: Some(long_title.clone()),
                ..Filters::default()
            },
            ..AskDraft::default()
        };
        shared.apply_intent(Intent::Ask(draft), &mut Vec::new());
        shared.library.catalogue =
            Loadable::Failed(sample::failure(FailureKind::Internal).with_hint(&long_hint));
        let mut harness = bar([width, 100.0], shared);
        harness.run();

        let panel =
            egui::Rect::from_min_size(egui::pos2(space::LG, space::LG), egui::vec2(width, 100.0));
        let first_row = [
            ("Question", harness.get_by_label("Question").rect()),
            ("Mode", harness.get_by_label("Mode").rect()),
            (
                "Stop",
                harness.get_by_role_and_label(Role::Button, "Stop").rect(),
            ),
        ];
        let second_row = [
            ("Books", harness.get_by_label("Books").rect()),
            ("Authors", harness.get_by_label("Authors").rect()),
            ("Tags", harness.get_by_label("Tags").rect()),
            ("the note", harness.get_by_label(&note).rect()),
            (
                "Try again",
                harness
                    .get_by_role_and_label(Role::Button, "Try again")
                    .rect(),
            ),
        ];
        for row in [&first_row[..], &second_row[..]] {
            for (name, rect) in row {
                assert!(
                    panel.contains_rect(*rect),
                    "{name} leaves the bar at {width}: {rect:?} is not inside {panel:?}"
                );
            }
            for pair in row.windows(2) {
                let ((left, a), (right, b)) = (pair[0], pair[1]);
                assert!(
                    a.right() < b.left(),
                    "{left} meets {right} at {width}: {a:?} and {b:?}"
                );
            }
        }
        if width < 1200.0 {
            testkit::save_png(&mut harness, "ask-bar-smallest");
        }
    }
}
