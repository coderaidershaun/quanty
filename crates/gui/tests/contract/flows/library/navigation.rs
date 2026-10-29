//! The shelf is narrowed with its find box, and the Source panel opens the page of its document in
//! the Library.

use std::sync::Arc;

use contract::{Intent, Tab};
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::state::Open;
use gui::testkit;
use gui::theme::space;

use super::{
    BLACK_SCHOLES, BOOK, HAWKES, Ingests, NOTES, PAPER, SAMPLE_PAGES, documents_of, is_open_row,
    open_document, open_switched,
};
use crate::flows::recording;
use crate::flows::{
    Window, click, field, has, hover_on, is_enabled, node, panels, retype, says, shared, type_into,
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

/// Presses the handle named `edge`, moves it by `by` points along the row and lets go.
fn drag_an_edge(harness: &mut Window, edge: &str, by: f32) {
    let from = node(harness, Role::Splitter, edge).rect().center();
    harness.drag_at(from);
    harness.run_ok();
    harness.hover_at(from + egui::vec2(by, 0.0));
    harness.run_ok();
    harness.drop_at(from + egui::vec2(by, 0.0));
    harness.run_ok();
}

fn edge_at(harness: &Window, edge: &str) -> f32 {
    node(harness, Role::Splitter, edge).rect().left()
}

fn drag_the_shelf_edge(harness: &mut Window, by: f32) {
    drag_an_edge(harness, "Resize the shelf", by);
}

fn shelf_edge(harness: &Window) -> f32 {
    edge_at(harness, "Resize the shelf")
}

fn drag_the_page_edge(harness: &mut Window, by: f32) {
    drag_an_edge(harness, "Resize the page", by);
}

fn page_edge(harness: &Window) -> f32 {
    edge_at(harness, "Resize the page")
}

#[test]
fn dragging_the_shelf_edge_resizes_it_within_bounds() {
    let (mut harness, _seen) = open_switched("black-scholes", &Arc::default(), Ingests::Run);
    click(&mut harness, Role::Tab, "Library");
    let start = shelf_edge(&harness);

    drag_the_shelf_edge(&mut harness, 100.0);
    let wider = shelf_edge(&harness);
    assert!(
        (wider - start - 100.0).abs() <= 8.0,
        "the edge followed the pointer from {start} to {wider}"
    );

    drag_the_shelf_edge(&mut harness, 4000.0);
    let widest = shelf_edge(&harness);
    drag_the_shelf_edge(&mut harness, 4000.0);
    assert_eq!(
        shelf_edge(&harness),
        widest,
        "the shelf stops at its widest"
    );
    assert!(widest > wider);

    drag_the_shelf_edge(&mut harness, -4000.0);
    let narrowest = shelf_edge(&harness);
    drag_the_shelf_edge(&mut harness, -4000.0);
    assert_eq!(
        shelf_edge(&harness),
        narrowest,
        "the shelf stops at its narrowest"
    );
    assert!(narrowest < start);
}

#[test]
fn dragging_the_page_edge_resizes_it_within_bounds() {
    const READER_MIN: f32 = 320.0;
    // Wide enough that a reader capped at the width of a page would leave a gap at the room's right.
    const WIDE_WINDOW: [f32; 2] = [2400.0, 1024.0];
    let (mut harness, _seen) = recording::open("black-scholes", WIDE_WINDOW);
    testkit::settle(&mut harness);
    click(&mut harness, Role::Tab, "Library");
    open_document(&mut harness, NOTES, BLACK_SCHOLES);
    let start = page_edge(&harness);

    drag_the_page_edge(&mut harness, 100.0);
    let wider = page_edge(&harness);
    assert!(
        (wider - start - 100.0).abs() <= 8.0,
        "the edge followed the pointer from {start} to {wider}"
    );

    drag_the_page_edge(&mut harness, 4000.0);
    let widest = page_edge(&harness);
    drag_the_page_edge(&mut harness, 4000.0);
    assert_eq!(page_edge(&harness), widest, "the page stops at its widest");
    assert!(widest > wider);
    let room = panels(WIDE_WINDOW).page;
    assert!(
        widest + space::MD <= room.right(),
        "the page edge {widest} stays inside the room that ends at {}",
        room.right()
    );

    drag_the_page_edge(&mut harness, -4000.0);
    let narrowest = page_edge(&harness);
    drag_the_page_edge(&mut harness, -4000.0);
    assert_eq!(
        page_edge(&harness),
        narrowest,
        "the page stops at its narrowest"
    );
    assert!(narrowest < start);

    click(&mut harness, Role::Button, "Read");
    testkit::settle(&mut harness);
    assert!(shared(&harness).shelf.is_reading);
    let beside = page_edge(&harness);
    drag_the_page_edge(&mut harness, 100.0);
    let wider = page_edge(&harness);
    assert!(
        (wider - beside - 100.0).abs() <= 8.0,
        "the edge followed the pointer from {beside} to {wider} with the reader beside"
    );

    drag_the_page_edge(&mut harness, 4000.0);
    let widest = page_edge(&harness);
    let reader_width = room.right() - widest - space::MD;
    assert!(
        (reader_width - READER_MIN).abs() <= 1.0,
        "the reader keeps its least width {READER_MIN}, not {reader_width}"
    );

    drag_the_page_edge(&mut harness, -4000.0);
    let reader_end = node(&harness, Role::Button, "Close reader").rect().right();
    assert!(
        room.right() - reader_end <= space::XL,
        "the reader fills the room that ends at {}, not up to {reader_end}",
        room.right()
    );
}
