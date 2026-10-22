//! The shelf is narrowed with its find box, and the Source panel opens the page of its document in
//! the Library.

use std::sync::Arc;

use contract::{Intent, Tab};
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::state::Open;
use gui::testkit;

use super::{
    BLACK_SCHOLES, BOOK, HAWKES, Ingests, NOTES, PAPER, SAMPLE_PAGES, documents_of, is_open_row,
    open_switched,
};
use crate::flows::recording;
use crate::flows::{
    Window, click, field, has, hover_on, is_enabled, retype, says, shared, type_into,
};

fn the_rows_are(harness: &Window, names: &[&str]) {
    let rows: Vec<String> = harness
        .query_all_by_role(Role::Button)
        .filter(|button| button.accesskit_node().toggled().is_some())
        .filter_map(|button| button.accesskit_node().label())
        .collect();
    assert_eq!(rows, names);
}

#[test]
fn the_find_box_narrows_the_shelf() {
    let (mut harness, _seen) = open_switched("black-scholes", &Arc::default(), Ingests::Run);
    click(&mut harness, Role::Tab, "Library");
    assert!(
        !has(&harness, Role::Button, "Clear"),
        "an empty box has nothing to clear"
    );

    type_into(&mut harness, "Find", "black");
    the_rows_are(&harness, &[NOTES, BLACK_SCHOLES]);
    assert!(has(&harness, Role::Button, "Clear"));
    assert_eq!(
        shared(&harness).shelf.open,
        Some(Open::Media {
            title: Some(HAWKES.to_owned())
        }),
        "the open page did not change"
    );
    assert!(has(&harness, Role::Label, HAWKES));

    retype(&mut harness, "Find", "OPTION");
    the_rows_are(
        &harness,
        &[
            BOOK,
            SAMPLE_PAGES,
            NOTES,
            "Chapter 1 · Options Pricing Intuition",
        ],
    );

    retype(&mut harness, "Find", "zzz");
    assert!(says(&harness, "Nothing matches"));
    the_rows_are(&harness, &[]);

    click(&mut harness, Role::Button, "Clear");
    assert_eq!(field(&harness, "Find").as_deref(), Some(""));
    the_rows_are(&harness, &[HAWKES, PAPER, BOOK, NOTES]);
    assert!(is_open_row(&harness, HAWKES));
    assert!(!has(&harness, Role::Button, "Clear"));

    type_into(&mut harness, "Find", "black");
    click(&mut harness, Role::Button, BLACK_SCHOLES);
    assert_eq!(
        field(&harness, "Find").as_deref(),
        Some("black"),
        "a click keeps the text"
    );
    assert!(is_open_row(&harness, BLACK_SCHOLES));
}

#[test]
fn open_in_library_shows_the_page_of_the_document_the_source_shows() {
    let (mut harness, _seen) = recording::open("idle", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    assert!(
        !has(&harness, Role::Button, "Open in Library"),
        "no page is open, so there is nothing to open in the Library"
    );

    let chapter = documents_of(&harness)
        .into_iter()
        .find(|document| document.name().label() == SAMPLE_PAGES)
        .expect("the book's chapter is listed")
        .id;
    harness.state_mut().push(Intent::OpenSource {
        doc: chapter,
        page: 1,
        piece: None,
    });
    testkit::settle(&mut harness);
    assert!(is_enabled(&harness, Role::Button, "Open in Library"));
    click(&mut harness, Role::Button, "Open in Library");
    assert_eq!(shared(&harness).tab, Tab::Library);
    assert_eq!(shared(&harness).shelf.open, Some(Open::Document(chapter)));
    assert!(
        is_open_row(&harness, SAMPLE_PAGES),
        "its row is the open one"
    );
    assert!(
        has(&harness, Role::Label, SAMPLE_PAGES),
        "its page names it"
    );

    let (mut harness, _seen) = recording::open("stores-down", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    assert!(
        has(&harness, Role::Button, "Open in Library"),
        "the Source panel shows a page"
    );
    assert!(
        !is_enabled(&harness, Role::Button, "Open in Library"),
        "the library is not read, so it lists no document"
    );
    hover_on(&mut harness, Role::Button, "Open in Library");
    assert!(says(
        &harness,
        "The library does not list this document yet."
    ));
}
