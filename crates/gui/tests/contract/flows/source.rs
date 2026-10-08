//! The Source panel shows the page its pickers and its pager name, turns to the next page with
//! no spinner in between, and shows a chapter with no page pictures as its pieces.

use std::time::{Duration, Instant};

use eframe::egui;
use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::contract::{DocId, Intent, Loadable, PieceKind};
use gui::testkit;

use super::{Window, click, has, is_enabled, node, press, says, shared};

const COMMAND: egui::Modifiers = egui::Modifiers::COMMAND;

fn shown_page(harness: &Window) -> Option<u32> {
    shared(harness).source.page.ready().map(|view| view.page)
}

fn page_label(harness: &Window) -> String {
    let view = shared(harness)
        .source
        .page
        .ready()
        .expect("a page is shown");
    format!("page {} of {}", view.page, view.page_count)
}

fn assert_page(harness: &Window, doc: DocId, page: u32) {
    let view = shared(harness)
        .source
        .page
        .ready()
        .unwrap_or_else(|| panic!("page {page} is not ready"));
    assert_eq!((view.doc, view.page), (doc, page));
    assert!(has(harness, Role::Label, &page_label(harness)));
}

fn book_of(harness: &Window, doc: DocId) -> String {
    let catalogue = shared(harness)
        .library
        .catalogue
        .ready()
        .expect("the library loaded");
    let book = catalogue.book_of(doc).expect("the document is in a book");
    book.title.clone().expect("the book has a title")
}

/// Queues a click and runs no frame, so that a test can look at every frame that follows.
fn queue_click(harness: &mut Window, role: Role, name: &str) {
    let at = node(harness, role, name).rect().center();
    harness.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
}

fn a_chapter_with_no_page_pictures_shows_its_pieces(harness: &mut Window, doc: DocId) {
    let notes_book = book_of(harness, doc);
    click(harness, Role::ComboBox, "Book");
    click(harness, Role::Button, &notes_book);
    click(harness, Role::ComboBox, "Chapter");
    let chapter = shared(harness)
        .library
        .catalogue
        .ready()
        .and_then(|catalogue| catalogue.document(doc))
        .and_then(|document| document.chapter.clone())
        .expect("the chapter of the formula has a label");
    click(
        harness,
        Role::Button,
        &format!("Chapter {}: {}", chapter.number, chapter.name),
    );
    assert_page(harness, doc, 1);
    let view = shared(harness)
        .source
        .page
        .ready()
        .expect("the page is ready")
        .clone();
    assert!(view.image.is_none(), "this chapter has no page pictures");
    assert!(!has(harness, Role::Image, "Picture of page 1"));
    let heading = view
        .pieces
        .iter()
        .find(|piece| matches!(piece.kind, PieceKind::Heading { .. }))
        .expect("the page has a heading");
    assert!(
        has(harness, Role::Label, &heading.text),
        "the pieces of the page are shown in place of its picture"
    );
    let source = &shared(harness).source;
    assert!(
        !matches!(source.page, Loadable::Failed(_))
            && !matches!(source.concepts, Loadable::Failed(_)),
        "nothing failed"
    );
}

#[test]
fn the_source_shows_the_page_its_pickers_and_pager_name() {
    let mut harness = testkit::app("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    let figure = shared(&harness)
        .ask
        .result(8)
        .expect("the figure result")
        .clone();
    let notes = shared(&harness)
        .ask
        .result(1)
        .expect("the formula result")
        .clone();
    let with_pictures = figure.doc;

    let book = book_of(&harness, with_pictures);
    click(&mut harness, Role::ComboBox, "Book");
    click(&mut harness, Role::Button, &book);
    assert_page(&harness, with_pictures, 1);
    assert!(has(&harness, Role::Image, "Picture of page 1"));

    click(&mut harness, Role::Button, "Next page");
    assert_page(&harness, with_pictures, 2);
    press(&mut harness, COMMAND, egui::Key::CloseBracket);
    assert_page(&harness, with_pictures, 3);
    press(&mut harness, COMMAND, egui::Key::OpenBracket);
    assert_page(&harness, with_pictures, 2);
    for _ in 0..10 {
        press(&mut harness, COMMAND, egui::Key::CloseBracket);
    }
    assert_page(&harness, with_pictures, 7);
    assert!(
        !is_enabled(&harness, Role::Button, "Next page"),
        "the last page has no next"
    );

    // The neighbour page is ready before it is asked for: the page that arrives is drawn with
    // its picture in the frame it arrives in.
    for _ in 0..3 {
        press(&mut harness, COMMAND, egui::Key::OpenBracket);
    }
    assert_page(&harness, with_pictures, 4);
    queue_click(&mut harness, Role::Button, "Next page");
    let started = Instant::now();
    loop {
        harness.step();
        if shown_page(&harness) == Some(5) {
            break;
        }
        assert!(
            has(&harness, Role::Image, "Picture of page 4"),
            "the old page stays until the next one arrives"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "page 5 never arrived"
        );
    }
    assert!(
        has(&harness, Role::Image, "Picture of page 5"),
        "the page that arrives has its picture in the same frame"
    );
    testkit::settle(&mut harness);

    harness
        .state_mut()
        .push(Intent::SelectResult(figure.number));
    testkit::settle(&mut harness);
    assert!(
        says(&harness, "on the page"),
        "the figure is framed on the page"
    );
    testkit::save_png(&mut harness, "app-source");

    a_chapter_with_no_page_pictures_shows_its_pieces(&mut harness, notes.doc);
}
