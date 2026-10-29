//! The Library in a phone and a tablet window: on a phone the shelf or one page fills the room and
//! Back goes step by step, the views of a chapter and its buttons can all be reached, and on a
//! tablet the shelf stands beside the page.

use contract::{AskView, DocumentView, Intent, Tab};
use eframe::egui::accesskit::Role;
use eframe::egui::{self, Rect};
use egui_kittest::Node;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::{PHONE_WINDOW, SMALLEST_WINDOW, TABLET_WINDOW};
use gui::state::Open;
use gui::testkit;
use gui::theme::size;
use gui::widgets::{Surface, panel_frame};

use super::{BLACK_SCHOLES, BOOK, NOTES, SAMPLE_PAGES, is_open_row};
use crate::flows::fit;
use crate::flows::{
    Wheel, Window, click, click_in, has, is_open_tab, node, node_in, open_tab, panels, says,
    scroll_to, shared, type_into,
};

const ABOUT_MEDIA: &str = "About this media";

fn open_library(window: [f32; 2]) -> Window {
    let mut harness = testkit::app("black-scholes", window);
    testkit::settle(&mut harness);
    open_tab(&mut harness, Tab::Library);
    harness
}

/// Where a control counts as in view: inside the frame of the page, which cuts off what lies
/// beyond its margin. A control may touch the edge of that room, and a fraction of a point is
/// the rounding of a place in a scroll area.
fn in_view(window: [f32; 2]) -> Rect {
    let page = panels(window).page;
    let margin = panel_frame(Surface::Sheet).total_margin();
    Rect::from_min_max(
        egui::pos2(page.left() + margin.left, page.top() + margin.top),
        egui::pos2(page.right() - margin.right, page.bottom() - margin.bottom),
    )
    .expand(fit::SLACK)
}

/// The rows of the shelf, the back row and the About row too: a list row is the one button that is
/// on or off.
fn shelf_rows(harness: &Window) -> Vec<Node<'_>> {
    harness
        .query_all_by_role(Role::Button)
        .filter(|button| button.accesskit_node().toggled().is_some())
        .collect()
}

fn shelf_row_names(harness: &Window) -> Vec<String> {
    shelf_rows(harness)
        .iter()
        .filter_map(|row| row.accesskit_node().label())
        .collect()
}

/// The control of the page with that name, which must lie in view.
fn rect_in_view(harness: &Window, window: [f32; 2], role: Role, name: &str) -> Rect {
    let page = panels(window).page;
    let rect = node_in(harness, role, name, page)
        .unwrap_or_else(|| panic!("the page has no {role:?} named `{name}`"))
        .rect();
    assert!(
        in_view(window).contains_rect(rect),
        "`{name}` {rect:?} is not in view"
    );
    rect
}

/// A button of the page that lies in view and is tall enough for a finger.
fn assert_touchable(harness: &Window, window: [f32; 2], name: &str) {
    let rect = rect_in_view(harness, window, Role::Button, name);
    fit::assert_finger_sized(name, rect, size::CONTROL_MD);
}

/// The name of the first button that reads a listed piece on in the Source panel.
fn read_on_name(harness: &Window) -> String {
    harness
        .query_all_by_role(Role::Button)
        .filter_map(|button| button.accesskit_node().label())
        .find(|name| name.starts_with("Read on"))
        .expect("a listing of the chapter has a button that reads on")
}

/// Turns the mouse wheel one step down while the pointer rests at `at`.
fn wheel_down_at(harness: &mut Window, at: egui::Pos2) {
    harness.hover_at(at);
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -120.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_ok();
}

/// Turns the mouse wheel over the strip of views until the tab lies in view. The views are visited
/// from the left to the right, so the wheel only scrolls that way.
fn wheel_tab_into_view(harness: &mut Window, window: [f32; 2], name: &str) {
    let view = in_view(window);
    for _ in 0..40 {
        if harness
            .query_all_by_role_and_label(Role::Tab, name)
            .any(|tab| view.contains_rect(tab.rect()))
        {
            return;
        }
        // Every tab of the strip stands on the row of the first one, also when it is scrolled out.
        let row = node(harness, Role::Tab, "Insights").rect().center().y;
        wheel_down_at(harness, egui::pos2(view.center().x, row));
    }
    panic!("the tab `{name}` never came into view");
}

/// The shelf is the whole page: its find box and rows lie in the room, tall enough for a finger,
/// and no page of a media or document is drawn.
fn assert_shelf_fills_the_page(harness: &Window, window: [f32; 2]) {
    let page = panels(window).page;
    assert!(
        has(harness, Role::TextInput, "Find"),
        "the find box is drawn"
    );
    let rows = shelf_rows(harness);
    assert!(!rows.is_empty(), "the shelf lists rows");
    let parts: Vec<(String, Rect)> = rows
        .iter()
        .map(|row| (row.accesskit_node().label().unwrap_or_default(), row.rect()))
        .collect();
    fit::assert_inside_and_apart(&parts, page, fit::window_rect(window));
    for (name, rect) in &parts {
        fit::assert_finger_sized(name, *rect, size::CONTROL_MD);
    }
    let page_buttons = harness
        .query_all_by_role(Role::Button)
        .filter_map(|button| button.accesskit_node().label())
        .filter(|name| name.starts_with("Delete media"))
        .count();
    assert_eq!(page_buttons, 0, "no media page is drawn");
    assert!(!says(harness, "Nothing is open"));
    assert!(
        node_in(harness, Role::Button, "Library", page).is_none(),
        "no back row is drawn"
    );
}

#[test]
fn a_phone_shows_the_shelf_until_a_page_is_opened() {
    let mut harness = open_library(PHONE_WINDOW);
    let page = panels(PHONE_WINDOW).page;
    assert_shelf_fills_the_page(&harness, PHONE_WINDOW);
    testkit::save_png(&mut harness, "phone-library-shelf");

    type_into(&mut harness, "Find", "black");
    assert_eq!(shelf_row_names(&harness), [NOTES, BLACK_SCHOLES]);
    assert_touchable(&harness, PHONE_WINDOW, "Clear");
    click(&mut harness, Role::Button, "Clear");

    click(&mut harness, Role::Button, BOOK);
    assert!(is_open_row(&harness, BOOK));
    assert!(
        shelf_row_names(&harness).contains(&SAMPLE_PAGES.to_owned()),
        "the chapters of the media unfold on the shelf"
    );
    assert!(!shelf_row_names(&harness).contains(&"Library".to_owned()));
    assert!(has(&harness, Role::TextInput, "Find"));

    click_in(&mut harness, Role::Button, ABOUT_MEDIA, page);
    assert!(
        node_in(&harness, Role::Button, "Library", page).is_some(),
        "the media page has a back row"
    );
    assert!(!has(&harness, Role::TextInput, "Find"));
    for name in [format!("Edit {BOOK}"), format!("Delete media {BOOK}")] {
        assert_touchable(&harness, PHONE_WINDOW, &name);
    }
    testkit::save_png(&mut harness, "phone-library-media");

    click_in(
        &mut harness,
        Role::Button,
        &format!("Delete media {BOOK}"),
        page,
    );
    let window = fit::window_rect(PHONE_WINDOW);
    for name in ["Delete media", "Cancel"] {
        let rect = node(&harness, Role::Button, name).rect();
        assert!(window.contains_rect(rect), "`{name}` {rect:?} is cut off");
    }
    testkit::save_png(&mut harness, "phone-library-delete-sheet");
    click(&mut harness, Role::Button, "Cancel");

    click_in(&mut harness, Role::Button, "Library", page);
    assert_eq!(shared(&harness).shelf.open, None);
    assert_shelf_fills_the_page(&harness, PHONE_WINDOW);
}

/// Read fills the phone with the reader and a back row, and Back leaves the reader for the page
/// of the chapter.
fn read_fills_the_phone_and_back_leaves_the_reader(harness: &mut Window, page: Rect) {
    click_in(harness, Role::Button, "Read", page);
    assert_eq!(
        shared(harness).tab,
        Tab::Library,
        "Read stays on the Library"
    );
    assert!(shared(harness).shelf.is_reading);
    assert!(
        node_in(harness, Role::Label, "Source in Context", page).is_some(),
        "the reader fills the phone"
    );
    assert!(
        node_in(harness, Role::Button, BLACK_SCHOLES, page).is_some(),
        "the reader has a back row"
    );
    assert!(
        !has(harness, Role::Tab, "Insights"),
        "the page is not drawn behind it"
    );
    testkit::save_png(harness, "phone-library-reader");
    click_in(harness, Role::Button, BLACK_SCHOLES, page);
    assert!(!shared(harness).shelf.is_reading, "Back closes the reader");
    assert!(
        has(harness, Role::Label, BLACK_SCHOLES),
        "the page of the chapter shows again"
    );
}

#[test]
fn a_phone_opens_every_view_of_a_chapter_and_goes_back_step_by_step() {
    let mut harness = open_library(PHONE_WINDOW);
    let page = panels(PHONE_WINDOW).page;
    click(&mut harness, Role::Button, NOTES);
    click(&mut harness, Role::Button, BLACK_SCHOLES);
    assert!(
        node_in(&harness, Role::Button, NOTES, page).is_some(),
        "the back row names the media"
    );
    assert!(has(&harness, Role::Label, BLACK_SCHOLES));
    assert!(!has(&harness, Role::TextInput, "Find"));

    let delete = format!("Delete document {BLACK_SCHOLES}");
    for name in ["Copy id", "Read", "Edit tags", &delete] {
        assert_touchable(&harness, PHONE_WINDOW, name);
    }
    testkit::save_png(&mut harness, "phone-library-controls");

    for view in DocumentView::ALL {
        let label = view.label();
        wheel_tab_into_view(&mut harness, PHONE_WINDOW, label);
        click_in(&mut harness, Role::Tab, label, page);
        assert_eq!(shared(&harness).shelf.view, view);
        assert!(is_open_tab(&harness, label), "`{label}` is the open view");
        rect_in_view(&harness, PHONE_WINDOW, Role::Tab, label);
        match view {
            DocumentView::Insights | DocumentView::Takeaways | DocumentView::CheatSheet => {
                assert_touchable(&harness, PHONE_WINDOW, "Export");
                if view == DocumentView::Insights {
                    assert_touchable(&harness, PHONE_WINDOW, "Page 2");
                }
                click_in(&mut harness, Role::Button, "Export", page);
                let item = node(&harness, Role::Button, "Export as PDF…").rect();
                fit::assert_finger_sized("Export as PDF…", item, size::CONTROL_MD);
                harness.key_press(egui::Key::Escape);
                testkit::settle(&mut harness);
            }
            DocumentView::Code => assert_touchable(&harness, PHONE_WINDOW, &read_on_name(&harness)),
            _ => {}
        }
        if matches!(
            view,
            DocumentView::Insights | DocumentView::Code | DocumentView::Concepts
        ) {
            testkit::save_png(
                &mut harness,
                &format!("phone-library-{}", label.to_lowercase()),
            );
        }
    }

    read_fills_the_phone_and_back_leaves_the_reader(&mut harness, page);

    click_in(&mut harness, Role::Button, NOTES, page);
    let media = Open::Media {
        title: Some(NOTES.to_owned()),
    };
    assert_eq!(shared(&harness).shelf.open, Some(media));
    assert!(has(&harness, Role::TextInput, "Find"));
    assert!(shelf_row_names(&harness).contains(&BLACK_SCHOLES.to_owned()));

    harness.state_mut().push(Intent::Back);
    testkit::settle(&mut harness);
    assert_eq!(shared(&harness).shelf.open, None);
    assert!(!shelf_row_names(&harness).contains(&BLACK_SCHOLES.to_owned()));

    harness.state_mut().push(Intent::RefreshCatalogue);
    testkit::settle(&mut harness);
    assert_eq!(
        shared(&harness).shelf.open,
        None,
        "a catalogue that is read does not open what Back closed"
    );

    harness.state_mut().push(Intent::Back);
    testkit::settle(&mut harness);
    assert_eq!(shared(&harness).tab, Tab::Ask);
}

#[test]
fn read_on_a_phone_opens_the_source_view_of_ask() {
    let mut harness = open_library(PHONE_WINDOW);
    let page = panels(PHONE_WINDOW).page;
    click(&mut harness, Role::Button, NOTES);
    click(&mut harness, Role::Button, BLACK_SCHOLES);
    wheel_tab_into_view(&mut harness, PHONE_WINDOW, "Code");
    click_in(&mut harness, Role::Tab, "Code", page);

    let read_on = |harness: &mut Window| {
        let button = harness
            .query_all_by_role(Role::Button)
            .find(|button| {
                let name = button.accesskit_node().label();
                name.is_some_and(|name| name.starts_with("Read on"))
            })
            .expect("a listing of the chapter has a button that reads on");
        assert!(in_view(PHONE_WINDOW).contains_rect(button.rect()));
        button.click();
        testkit::settle(harness);
    };
    let library_page = shared(&harness).shelf.clone();
    read_on(&mut harness);
    assert_eq!(shared(&harness).tab, Tab::Ask);
    assert_eq!(shared(&harness).ask_view, AskView::Source);
    let source = panels(PHONE_WINDOW).source;
    assert!(
        node_in(&harness, Role::Label, "Source in Context", source).is_some(),
        "the Source view fills the phone"
    );

    open_tab(&mut harness, Tab::Library);
    assert!(
        node_in(&harness, Role::Button, NOTES, page).is_some(),
        "the Library shows the chapter it had open"
    );

    read_on(&mut harness);
    assert!(
        harness.state().can_go_back(),
        "a Source view that was read on can go back"
    );
    harness.state_mut().push(Intent::Back);
    testkit::settle(&mut harness);
    assert_eq!(shared(&harness).tab, Tab::Library);
    assert_eq!(
        shared(&harness).shelf,
        library_page,
        "Back returns to the page that Read on left"
    );
    assert!(
        node_in(&harness, Role::Button, NOTES, page).is_some(),
        "the back row names the media again"
    );
    assert!(is_open_tab(&harness, "Code"), "the same view is open");
    assert!(
        harness.state().can_go_back(),
        "a chapter is open, so the Library can go back"
    );
}

// SMELL: this file is over 400 lines. The tests of a tablet and of the smallest window from here
// down are the part to move out first, so that this file keeps the journeys of a phone.
#[test]
fn a_tablet_shows_the_shelf_beside_the_page() {
    let mut harness = open_library(TABLET_WINDOW);
    click(&mut harness, Role::Button, NOTES);
    click(&mut harness, Role::Button, BLACK_SCHOLES);
    let window = fit::window_rect(TABLET_WINDOW);

    let mut shelf = vec![node(&harness, Role::TextInput, "Find").rect()];
    // The page chips of the Insights view are on or off like a row of the shelf.
    let is_chip = |row: &Node<'_>| {
        let name = row.accesskit_node().label();
        name.is_some_and(|name| name.starts_with("Page "))
    };
    shelf.extend(
        shelf_rows(&harness)
            .iter()
            .filter(|row| !is_chip(row))
            .map(Node::rect),
    );
    let mut page = vec![node(&harness, Role::Label, BLACK_SCHOLES).rect()];
    let delete = format!("Delete document {BLACK_SCHOLES}");
    for name in ["Copy id", "Read", "Edit tags", &delete] {
        page.push(node(&harness, Role::Button, name).rect());
    }
    let insights = node(&harness, Role::Tab, "Insights").rect();
    // The views at the right end of the strip may be scrolled out of the window.
    let strip: Vec<Rect> = DocumentView::ALL
        .iter()
        .map(|view| node(&harness, Role::Tab, view.label()).rect())
        .collect();

    let shelf_right = shelf
        .iter()
        .map(|rect| rect.right())
        .fold(f32::MIN, f32::max);
    let page_left = page
        .iter()
        .chain(&strip)
        .map(|rect| rect.left())
        .fold(f32::MAX, f32::min);
    assert!(
        shelf_right <= page_left,
        "the shelf ends at {shelf_right}, and the page starts at {page_left}"
    );
    for rect in shelf.iter().chain(&page).chain([&insights]) {
        assert!(
            window.contains_rect(*rect),
            "{rect:?} is outside the window"
        );
    }
    let named_notes = harness
        .query_all_by_role_and_label(Role::Button, NOTES)
        .count();
    assert_eq!(named_notes, 1, "no back row stands beside the shelf");
    testkit::save_png(&mut harness, "tablet-library");
}

#[test]
fn the_delete_sheet_fits_a_360_pt_window() {
    let mut harness = open_library(SMALLEST_WINDOW);
    let page = panels(SMALLEST_WINDOW).page;
    scroll_to(
        &mut harness,
        in_view(SMALLEST_WINDOW),
        Wheel::Down,
        Role::Button,
        BOOK,
    );
    click(&mut harness, Role::Button, BOOK);
    click_in(&mut harness, Role::Button, ABOUT_MEDIA, page);
    click_in(
        &mut harness,
        Role::Button,
        &format!("Delete media {BOOK}"),
        page,
    );

    let window = fit::window_rect(SMALLEST_WINDOW);
    let sheet = harness.ctx.memory(|memory| {
        let layer = memory.top_modal_layer()?;
        memory.area_rect(layer.id)
    });
    let sheet = sheet.expect("the question is a sheet over the window");
    assert!(
        window.contains_rect(sheet),
        "the sheet {sheet:?} is cut off"
    );
    let title = format!("Delete the media {BOOK}?");
    let body = harness
        .query_all_by_label_contains("This cannot be undone.")
        .next()
        .expect("the sheet says what goes");
    for (name, rect) in [
        ("the title", node(&harness, Role::Label, &title).rect()),
        ("the body", body.rect()),
        (
            "Delete media",
            node(&harness, Role::Button, "Delete media").rect(),
        ),
        ("Cancel", node(&harness, Role::Button, "Cancel").rect()),
    ] {
        assert!(window.contains_rect(rect), "{name} {rect:?} is cut off");
    }
    testkit::save_png(&mut harness, "small-library-delete-sheet");
    click(&mut harness, Role::Button, "Cancel");
}
