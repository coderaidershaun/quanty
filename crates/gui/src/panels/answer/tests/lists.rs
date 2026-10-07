//! The lists of results: each tab, and the way a selected result is brought into view.

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;

use super::{
    SMALL, TALL, WIDE, answered, apply, found, look, open_tab, pane, says, shown_results, written,
};
use crate::contract::{Intent, SearchReply};
use crate::panels::answer::listing::Listing;
use crate::testkit::{self, sample};
use crate::theme::space;

/// egui spreads one turn of the wheel over a few frames. This many frames end it.
const WHEEL_FRAMES: usize = 8;

#[test]
fn each_tab_lists_the_results_of_its_kind_under_their_own_numbers() {
    let counts = Listing::of(&found().results).counts;
    assert_eq!(
        (
            counts.results,
            counts.formulas,
            counts.figures,
            counts.tables
        ),
        (5, 1, 1, 1)
    );

    let mut harness = pane(TALL, answered());
    harness.run();
    for (tab, numbers) in [
        ("Results", vec![1, 2, 3, 4, 5]),
        ("Formulas", vec![1]),
        ("Figures", vec![4]),
        ("Tables", vec![5]),
    ] {
        open_tab(&mut harness, tab);
        assert_eq!(shown_results(&harness), numbers, "the {tab} tab");
        if tab == "Results" {
            assert!(says(&harness, "Cited by result 2 as Table 1-1"));
            assert!(says(&harness, "Reached through the concept Volatility"));
        }
        look(
            &format!("answer-{}", tab.to_lowercase()),
            answered(),
            Some(tab),
        );
    }

    let without_table = SearchReply {
        results: found().results[..4].to_vec(),
        ..SearchReply::default()
    };
    let shared = testkit::answered(without_table, sample::concept_graph(), written());
    let mut harness = pane(WIDE, shared);
    harness.run();
    open_tab(&mut harness, "Tables");
    assert!(says(&harness, "No tables among the results"));
    assert!(shown_results(&harness).is_empty());
    testkit::save_png(&mut harness, "answer-kind-empty");
}

#[test]
fn a_result_selected_elsewhere_is_scrolled_into_view_and_rows_off_screen_are_not_drawn() {
    let panel =
        egui::Rect::from_min_size(egui::pos2(space::LG, space::LG), egui::Vec2::from(SMALL));
    let mut harness = pane(SMALL, testkit::searched(found()));
    harness.run();
    open_tab(&mut harness, "Results");
    let tabs = harness.get_by_role_and_label(Role::Tab, "Results").rect();
    let list = egui::Rect::from_min_max(egui::pos2(panel.left(), tabs.bottom()), panel.max);
    assert!(harness.query_by_label("Result 1").is_some());
    assert!(
        harness.query_by_label("Result 5").is_none(),
        "a row off screen is not drawn"
    );

    apply(&mut harness.state_mut().shared, Intent::SelectResult(5));
    harness.run();
    harness.state_mut().media.run_pending();
    harness.run();
    let row = harness.get_by_label("Result 5").rect();
    assert!(
        list.contains_rect(row),
        "the selected row is in the pane, under the tabs: {row:?} not in {list:?}"
    );
    assert!(
        harness.query_by_label("Result 1").is_none(),
        "the rows it passed are gone"
    );
    testkit::save_png(&mut harness, "answer-results-selected");

    apply(&mut harness.state_mut().shared, Intent::SelectResult(4));
    harness.run();
    let row = harness.get_by_label("Result 4").rect();
    assert!(
        list.contains_rect(row),
        "a row that the top of the list cut is brought in whole"
    );

    apply(&mut harness.state_mut().shared, Intent::SelectResult(1));
    harness.run();
    let row = harness.get_by_label("Result 1").rect();
    assert!(list.contains_rect(row), "the way back works too");

    harness.hover_at(list.center());
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -list.height()),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::default(),
    });
    harness.run_steps(WHEEL_FRAMES);
    assert!(
        harness.query_by_label("Result 1").is_none(),
        "a row that was brought into view is left alone: the wheel moves the list off it"
    );

    let mut small = pane(SMALL, answered());
    small.run();
    small.state_mut().media.run_pending();
    small.run();
    testkit::save_png(&mut small, "answer-ready-small");
}
