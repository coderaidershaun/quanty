//! The Ingest and Datasets forms in a phone, in a tablet and in the smallest window: every control
//! can be scrolled into view, lies inside the column from side to side, and covers no other
//! control or text, and the journeys of the forms still work with the pointer.

use contract::{Category, Command, DatasetRequest, DocId, IngestStage, NewMedia, Tab};
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers, Rect, pos2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::{PHONE_WINDOW, SMALLEST_WINDOW, TABLET_WINDOW};
use gui::state::{DatasetJob, IngestJob, Shared};
use gui::testkit::{self, run_until};
use gui::theme::size;
use uuid::Uuid;

use crate::flows::fit::{self, SLACK};
use crate::flows::recording::{self, Seen};
use crate::flows::{
    CHAPTER_2, DRY_RUN, Window, click, is_at_its_stall, is_preflight, node, open_tab, panels,
    press, says, sent_builds, shared, type_into,
};

/// What a form is made of: its controls and its texts.
const SEEN: [Role; 5] = [
    Role::Button,
    Role::TextInput,
    Role::ComboBox,
    Role::CheckBox,
    Role::Label,
];

const ADD_MEDIA: &str = "Add media";
const BUILD_A_DATASET: &str = "Build a training dataset";
const OPTION_VOLATILITY_PENCIL: &str = "Edit Option Volatility and Pricing";
const ROUGH_VOLATILITY: &str = "Rough Volatility";
const ROUGH_VOLATILITY_PENCIL: &str = "Edit Rough Volatility";
const ROW_OF_A_NEW_MEDIA: &str = "Add new media…";
const NOTE_OF_A_NEW_MEDIA: &str =
    "A saved media stays in this list with its authors and tags, also before its first PDF.";

const INGEST_READY: &[(Role, &str)] = &[
    (Role::ComboBox, "Media"),
    (Role::Button, OPTION_VOLATILITY_PENCIL),
    (Role::Button, "Choose a PDF"),
    (Role::Button, "Chapter"),
    (Role::Button, "Other part"),
    (Role::TextInput, "Chapter number"),
    (Role::TextInput, "Chapter name"),
    (Role::TextInput, "Tags for this PDF"),
    (Role::Button, "Start ingest"),
];
const NEW_MEDIA: &[(Role, &str)] = &[
    (Role::Button, "Book"),
    (Role::Button, "Paper"),
    (Role::Button, "Other"),
    (Role::TextInput, "Media title"),
    (Role::TextInput, "Authors"),
    (Role::TextInput, "Tags"),
    (Role::Button, "Save media"),
    (Role::Button, "Cancel"),
    (Role::Button, "Choose a PDF"),
    (Role::TextInput, "Tags for this PDF"),
];
const EDIT_MEDIA: &[(Role, &str)] = &[
    (Role::Button, "Book"),
    (Role::Button, "Paper"),
    (Role::Button, "Other"),
    (Role::TextInput, "Authors"),
    (Role::TextInput, "Tags"),
    (Role::Button, "Save media"),
    (Role::Button, "Cancel"),
];
const DATASETS_READY: &[(Role, &str)] = &[
    (Role::TextInput, "Dataset name"),
    (Role::ComboBox, "Media"),
    (Role::ComboBox, "Hold out"),
    (Role::Button, "Fewer rewrites per section"),
    (Role::Button, "More rewrites per section"),
    (Role::Button, "Fewer questions per section"),
    (Role::Button, "More questions per section"),
    (Role::Button, "Fewer SFT examples at most"),
    (Role::Button, "More SFT examples at most"),
    (Role::CheckBox, DRY_RUN),
    (Role::Button, "Start build"),
];

fn open(scene: &str, window: [f32; 2]) -> (Window, Seen) {
    let (mut harness, seen) = recording::open(scene, window);
    testkit::settle(&mut harness);
    (harness, seen)
}

/// Where the content of the column shows. The column is centred in the page and as tall as the
/// page, so its margins are those of the title. Measure it on a page that is not scrolled yet.
fn inside_of_the_column(harness: &Window, window: [f32; 2], title: &str) -> Rect {
    let page = panels(window).page;
    let title = node(harness, Role::Label, title).rect();
    Rect::from_min_max(
        title.min,
        pos2(
            page.left() + page.right() - title.left(),
            page.bottom() - (title.top() - page.top()),
        ),
    )
}

fn shares_height(rect: Rect, inside: Rect) -> bool {
    rect.bottom() > inside.top() + SLACK && rect.top() < inside.bottom() - SLACK
}

/// The column from side to side with no limit above or below, because a form scrolls.
fn strip_of(inside: Rect) -> Rect {
    Rect::from_x_y_ranges(inside.x_range(), f32::NEG_INFINITY..=f32::INFINITY)
}

/// A node that is off screen has its rect all the same, so the page is scrolled until it shows.
fn bring_into_view(harness: &mut Window, role: Role, name: &str) {
    node(harness, role, name).scroll_to_me();
    harness.run_ok();
}

/// Every control and text that shows in `inside` lies within it from side to side, and no two of
/// them overlap. A notice is a label around labels, so none may show where this is used.
fn assert_nothing_overlaps(harness: &Window, inside: Rect) {
    let mut shown: Vec<(String, Rect)> = Vec::new();
    for role in SEEN {
        for found in harness.query_all_by_role(role) {
            let rect = found.rect();
            if shares_height(rect, inside) {
                let name = found
                    .accesskit_node()
                    .label()
                    .or_else(|| found.value())
                    .unwrap_or_default();
                shown.push((format!("{role:?} {name}"), rect));
            }
        }
    }
    fit::assert_inside_and_apart(&shown, strip_of(inside), Rect::EVERYTHING);
}

/// A check of what shows proves nothing about a node that is still scrolled away, so each one
/// first makes sure that its node came wholly into view.
fn assert_in_view(harness: &Window, inside: Rect, role: Role, name: &str) -> Rect {
    let rect = node(harness, role, name).rect();
    assert!(
        rect.top() >= inside.top() - SLACK && rect.bottom() <= inside.bottom() + SLACK,
        "{role:?} `{name}` at {rect:?} did not come wholly into view in {inside:?}"
    );
    rect
}

/// Each control in turn is scrolled into view, and must fit the column from top to bottom and
/// from side to side, and have nothing over it.
fn walk(harness: &mut Window, inside: Rect, controls: &[(Role, &str)]) {
    for &(role, name) in controls {
        bring_into_view(harness, role, name);
        let rect = assert_in_view(harness, inside, role, name);
        let part = (format!("{role:?} {name}"), rect);
        fit::assert_inside_and_apart(&[part], strip_of(inside), Rect::EVERYTHING);
        assert_nothing_overlaps(harness, inside);
    }
}

fn click_in_view(harness: &mut Window, role: Role, name: &str) {
    bring_into_view(harness, role, name);
    click(harness, role, name);
}

/// The chips and steppers of a form in a narrow column are as tall as a finger.
fn assert_finger_sized_in_view(harness: &mut Window, controls: &[(Role, &str)]) {
    for &(role, name) in controls {
        bring_into_view(harness, role, name);
        fit::assert_finger_sized(name, node(harness, role, name).rect(), size::CONTROL_MD);
    }
}

fn type_in_view(harness: &mut Window, name: &str, text: &str) {
    bring_into_view(harness, Role::TextInput, name);
    type_into(harness, name, text);
}

/// A row of an open list is clicked with the pointer, so it must be on the screen.
fn click_a_row_in_the_window(harness: &mut Window, window: [f32; 2], role: Role, name: &str) {
    let rect = node(harness, role, name).rect();
    assert!(
        fit::window_rect(window).contains_rect(rect),
        "the row `{name}` at {rect:?} is not inside the window"
    );
    click(harness, role, name);
}

fn a_checked_chapter_is_started_at_a_phone() {
    let (mut harness, seen) = open("ingest-ready", PHONE_WINDOW);
    assert_eq!(seen.count(is_preflight), 1, "the chapter is checked");
    assert!(says(
        &harness,
        "Chapter 3 · Greeks — 12 pages. Not converted yet."
    ));
    testkit::save_png(&mut harness, "phone-ingest-ready");

    let inside = inside_of_the_column(&harness, PHONE_WINDOW, ADD_MEDIA);
    walk(&mut harness, inside, INGEST_READY);
    assert_finger_sized_in_view(
        &mut harness,
        &[(Role::Button, "Chapter"), (Role::Button, "Other part")],
    );

    let number = node(&harness, Role::TextInput, "Chapter number").rect();
    let name = node(&harness, Role::TextInput, "Chapter name").rect();
    assert!(
        number.bottom() <= name.top(),
        "the chapter name {name:?} is not under the chapter number {number:?}"
    );

    click_in_view(&mut harness, Role::Button, "Start ingest");
    let IngestJob::Finished { result, .. } = &shared(&harness).ingest else {
        panic!("the ingest did not finish: {:?}", shared(&harness).ingest);
    };
    assert!(result.is_ok(), "{result:?}");
    assert!(says(
        &harness,
        "Ingested Option Volatility and Pricing, chapter 3: Greeks"
    ));
    testkit::save_png(&mut harness, "phone-ingest-done");
}

fn a_new_media_is_typed_and_saved_at_a_phone() {
    let (mut harness, seen) = open("idle", PHONE_WINDOW);
    open_tab(&mut harness, Tab::Ingest);
    let inside = inside_of_the_column(&harness, PHONE_WINDOW, ADD_MEDIA);

    click(&mut harness, Role::ComboBox, "Media");
    click_a_row_in_the_window(&mut harness, PHONE_WINDOW, Role::Button, ROW_OF_A_NEW_MEDIA);
    walk(&mut harness, inside, NEW_MEDIA);
    assert_finger_sized_in_view(
        &mut harness,
        &[
            (Role::Button, "Book"),
            (Role::Button, "Paper"),
            (Role::Button, "Other"),
        ],
    );
    // Beside the buttons the note would be cut short, so it stands whole under them.
    let buttons = node(&harness, Role::Button, "Save media").rect();
    let note = node(&harness, Role::Label, NOTE_OF_A_NEW_MEDIA).rect();
    assert!(
        buttons.bottom() <= note.top() + SLACK,
        "the note {note:?} is not under the buttons {buttons:?}"
    );

    click_in_view(&mut harness, Role::Button, "Paper");
    type_in_view(&mut harness, "Media title", ROUGH_VOLATILITY);
    type_in_view(&mut harness, "Authors", "A. Author, B. Author");
    type_in_view(&mut harness, "Tags", "Volatility, rough");
    testkit::save_png(&mut harness, "phone-ingest-new-media");
    click_in_view(&mut harness, Role::Button, "Save media");

    let saved: Vec<NewMedia> = seen
        .all()
        .into_iter()
        .filter_map(|command| match command {
            Command::SaveMedia { media, .. } => Some(media),
            _ => None,
        })
        .collect();
    let wanted = NewMedia {
        title: ROUGH_VOLATILITY.to_owned(),
        category: Category::Paper,
        authors: vec!["A. Author".to_owned(), "B. Author".to_owned()],
        tags: vec!["Volatility".to_owned(), "rough".to_owned()],
    };
    assert_eq!(saved, [wanted]);
    let catalogue = shared(&harness).library.catalogue.ready();
    assert!(
        catalogue.is_some_and(|catalogue| catalogue.media_titled(ROUGH_VOLATILITY).is_some()),
        "the catalogue does not hold the saved media"
    );

    bring_into_view(&mut harness, Role::Button, ROUGH_VOLATILITY_PENCIL);
    assert_in_view(&harness, inside, Role::Button, ROUGH_VOLATILITY_PENCIL);
    assert_nothing_overlaps(&harness, inside);
    testkit::save_png(&mut harness, "phone-ingest-saved-media");
    click(&mut harness, Role::Button, ROUGH_VOLATILITY_PENCIL);
    walk(&mut harness, inside, EDIT_MEDIA);
    testkit::save_png(&mut harness, "phone-ingest-edit-media");
}

#[test]
fn the_ingest_form_works_on_a_phone() {
    a_checked_chapter_is_started_at_a_phone();
    a_new_media_is_typed_and_saved_at_a_phone();
}

// SMELL: this file is over 400 lines. The Datasets journey and the tests of a tablet and of the
// smallest window from here down are the part to move out first, so that this file keeps the
// Ingest journeys and the checks that both use.
#[test]
fn the_datasets_form_works_on_a_phone() {
    let (mut harness, seen) = open("datasets-ready", PHONE_WINDOW);
    testkit::save_png(&mut harness, "phone-datasets-ready");
    let inside = inside_of_the_column(&harness, PHONE_WINDOW, BUILD_A_DATASET);
    walk(&mut harness, inside, DATASETS_READY);
    assert_finger_sized_in_view(
        &mut harness,
        &[
            (Role::Button, "Fewer rewrites per section"),
            (Role::Button, "More rewrites per section"),
            (Role::Button, "Fewer questions per section"),
            (Role::Button, "More questions per section"),
            (Role::Button, "Fewer SFT examples at most"),
            (Role::Button, "More SFT examples at most"),
        ],
    );

    // While every media is chosen, the rows of the list are the longest: a media and a chapter.
    click_in_view(&mut harness, Role::ComboBox, "Hold out");
    let window = fit::window_rect(PHONE_WINDOW);
    let rows: Vec<(String, Rect)> = harness
        .query_all_by_role(Role::CheckBox)
        .map(|row| (row.accesskit_node().label().unwrap_or_default(), row.rect()))
        .filter(|(name, _)| name != DRY_RUN)
        .collect();
    assert!(
        !rows.is_empty(),
        "the list of documents to hold out is empty"
    );
    for (name, rect) in &rows {
        assert!(
            window.contains_rect(*rect),
            "the row `{name}` at {rect:?} is not inside the window {window:?}"
        );
    }
    testkit::save_png(&mut harness, "phone-datasets-hold-out");
    press(&mut harness, Modifiers::NONE, Key::Escape);

    type_in_view(&mut harness, "Dataset name", "notes-v2");
    click_in_view(&mut harness, Role::ComboBox, "Media");
    click_a_row_in_the_window(
        &mut harness,
        PHONE_WINDOW,
        Role::Button,
        "Quanty Sample Notes",
    );
    click_in_view(&mut harness, Role::ComboBox, "Hold out");
    click_a_row_in_the_window(&mut harness, PHONE_WINDOW, Role::CheckBox, CHAPTER_2);
    press(&mut harness, Modifiers::NONE, Key::Escape);
    click_in_view(&mut harness, Role::Button, "More rewrites per section");
    testkit::save_png(&mut harness, "phone-datasets-filled");
    click_in_view(&mut harness, Role::Button, "Start build");

    let wanted = DatasetRequest {
        name: "notes-v2".to_owned(),
        media: Some("Quanty Sample Notes".to_owned()),
        holdout: vec![DocId(Uuid::from_u128(2))],
        rewrites: 4,
        questions: 2,
        max_sft: 300,
        dry_run: false,
    };
    assert_eq!(sent_builds(&seen), [wanted]);
    let DatasetJob::Finished { result, .. } = &shared(&harness).datasets else {
        panic!("the build did not finish: {:?}", shared(&harness).datasets);
    };
    assert!(result.is_ok(), "{result:?}");
    assert!(says(&harness, "Built notes-v2"));
}

fn the_resting_forms_fit(window: [f32; 2], name: &str) {
    let forms = [
        ("ingest-ready", ADD_MEDIA, "ingest", INGEST_READY),
        (
            "datasets-ready",
            BUILD_A_DATASET,
            "datasets",
            DATASETS_READY,
        ),
    ];
    for (scene, title, page, controls) in forms {
        let mut harness = testkit::app(scene, window);
        testkit::settle(&mut harness);
        testkit::save_png(&mut harness, &format!("{name}-{page}-ready"));
        let inside = inside_of_the_column(&harness, window, title);
        walk(&mut harness, inside, controls);
    }
}

fn the_stage_words_of_a_running_build_fit_the_smallest_window() {
    let mut harness = testkit::app("datasets-running", SMALLEST_WINDOW);
    // The build never ends, so the app is never idle, and `settle` would wait for ever.
    run_until(&mut harness, "the stall of the build", is_at_its_stall);
    harness.run_ok();
    let inside = inside_of_the_column(&harness, SMALLEST_WINDOW, BUILD_A_DATASET);
    let words = "Writing the synthetic texts — 6 of 12, about 50%";
    bring_into_view(&mut harness, Role::Label, words);
    assert_in_view(&harness, inside, Role::Label, words);
    assert_nothing_overlaps(&harness, inside);
    testkit::save_png(&mut harness, "smallest-datasets-running");
}

fn is_at_page_3(shared: &Shared) -> bool {
    matches!(
        &shared.ingest,
        IngestJob::Running { progress: Some(progress), .. }
            if matches!(progress.stage, IngestStage::Converting { saved: 3, .. })
    )
}

fn the_stage_words_of_a_running_ingest_fit_the_smallest_window() {
    let mut harness = testkit::app("ingest-running", SMALLEST_WINDOW);
    // This scene rests until Start is pressed, and then never ends.
    testkit::settle(&mut harness);
    let inside = inside_of_the_column(&harness, SMALLEST_WINDOW, ADD_MEDIA);
    bring_into_view(&mut harness, Role::Button, "Start ingest");
    // The ingest never ends, so the app is never idle again, and `click` would wait for ever.
    node(&harness, Role::Button, "Start ingest").click();
    run_until(&mut harness, "page 3", is_at_page_3);
    harness.run_ok();
    let words = "Converting the pages — 3 of 12 done, about 36%";
    bring_into_view(&mut harness, Role::Label, words);
    assert_in_view(&harness, inside, Role::Label, words);
    assert_nothing_overlaps(&harness, inside);
    testkit::save_png(&mut harness, "smallest-ingest-running");
}

#[test]
fn both_forms_fit_a_tablet_and_the_smallest_window() {
    the_resting_forms_fit(TABLET_WINDOW, "tablet");
    the_resting_forms_fit(SMALLEST_WINDOW, "smallest");
    the_stage_words_of_a_running_build_fit_the_smallest_window();
    the_stage_words_of_a_running_ingest_fit_the_smallest_window();
}
