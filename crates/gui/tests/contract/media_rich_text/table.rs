//! Checks tables of stored text: each column is as wide as what it holds, and a table that does
//! not fit scrolls sideways.

use eframe::egui;
use egui_kittest::kittest::Queryable;
use gui::media::rich_text;
use gui::testkit;
use gui::theme::TextRole;

use super::{LAST_HEADER, Painted, TABLE, painted, route};

fn table_in(width: f32, picture: &str) -> (egui::Rect, Vec<Painted>) {
    let mut harness = testkit::panel([width, 600.0], testkit::asked("q"), |ui, cx| {
        route(
            rich_text::table(ui, cx.media, TABLE, TextRole::Body),
            cx.intents,
        );
    });
    harness.run();
    let node = harness.get_by_label_contains(LAST_HEADER).rect();
    let shapes = painted(&harness);
    testkit::save_png(&mut harness, picture);
    (node, shapes)
}

#[test]
fn a_wide_table_scrolls_sideways_and_a_narrow_one_fits() {
    let furthest = |pieces: &[&Painted]| {
        pieces
            .iter()
            .map(|piece| piece.right)
            .fold(f32::MIN, f32::max)
    };
    let header_font = TextRole::Small.strong_font();

    let (node, shapes) = table_in(900.0, "rich-text-table-wide");
    assert!(node.width() <= 900.0 + 0.5, "the table fits its panel");
    let header: Vec<&Painted> = shapes
        .iter()
        .filter(|piece| piece.sections.iter().all(|(_, f)| f.font_id == header_font))
        .collect();
    assert!(
        squeezed_of(&header).ends_with("Ifforeignratesfall"),
        "the last header cell is drawn after the others"
    );
    let all: Vec<&Painted> = shapes.iter().collect();
    assert!(
        furthest(&all) <= node.right() + 0.5,
        "nothing of the table is scrolled out of its node"
    );

    let (narrow, shapes) = table_in(220.0, "rich-text-table-narrow");
    assert!(
        narrow.width() <= 220.0 + 0.5,
        "a narrow panel is not overflowed"
    );
    let all: Vec<&Painted> = shapes.iter().collect();
    assert!(
        furthest(&all) > narrow.right() + 0.5,
        "the columns that do not fit are scrolled out of the node, not squeezed"
    );
}

fn squeezed_of(pieces: &[&Painted]) -> String {
    pieces
        .iter()
        .flat_map(|piece| piece.text.chars())
        .filter(|c| !c.is_whitespace())
        .collect()
}

#[test]
fn each_row_of_a_wrapped_cell_is_aligned_on_its_own() {
    const NOTE: &str =
        "a note long enough to run onto a second row, which is shorter than the first row";
    let markdown = format!("| Case | Note |\n|---|---:|\n| 7 | {NOTE} |");
    let mut harness = testkit::panel([500.0, 300.0], testkit::asked("q"), move |ui, cx| {
        route(
            rich_text::table(ui, cx.media, &markdown, TextRole::Body),
            cx.intents,
        );
    });
    harness.run();
    let rights: Vec<f32> = painted(&harness)
        .iter()
        .filter(|piece| !piece.text.trim().is_empty() && NOTE.contains(piece.text.trim()))
        .map(|piece| piece.right)
        .collect();
    assert!(rights.len() >= 2, "the note wraps onto two rows");
    let spread = rights.iter().copied().fold(f32::MIN, f32::max)
        - rights.iter().copied().fold(f32::MAX, f32::min);
    assert!(
        spread <= 1.0,
        "every row of a cell set to the right ends at the same place: {rights:?}"
    );
}

#[test]
fn two_equal_tables_scroll_apart() {
    let mut harness = testkit::panel([300.0, 2000.0], testkit::asked("q"), |ui, cx| {
        for _ in 0..2 {
            route(
                rich_text::table(ui, cx.media, TABLE, TextRole::Body),
                cx.intents,
            );
        }
    });
    harness.run();
    assert_eq!(
        harness.query_all_by_label_contains(LAST_HEADER).count(),
        2,
        "each table names its own node"
    );
    let before: Vec<f32> = painted(&harness).iter().map(|piece| piece.right).collect();
    let half = before.len() / 2;

    let first = harness
        .query_all_by_label_contains(LAST_HEADER)
        .next()
        .expect("the first table")
        .rect();
    harness.hover_at(first.center());
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(-80.0, 0.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    let after: Vec<f32> = painted(&harness).iter().map(|piece| piece.right).collect();
    assert!(after[0] < before[0] - 1.0, "the first table scrolls");
    assert_eq!(
        after[half..],
        before[half..],
        "the second table stays where it was"
    );
}
