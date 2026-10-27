//! The views of the Ask tab on a phone and on a tablet: a phone shows one view at a time under a
//! bottom navigation, and a tablet shows the Answer beside the side panel that was chosen.

use contract::{AskView, Intent};
use eframe::egui::Rect;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::NodeT as _;
use gui::app::layout::{PHONE_WINDOW, ShellRects, TABLET_WINDOW};
use gui::testkit;
use gui::theme::size;

use crate::flows::fit;
use crate::flows::{Wheel, Window, click_in, has, node_in, panels, scroll_to, shared as state_of};

/// What each view is called on the bottom navigation and on the strip over the side panel.
const VIEW_LABELS: [(AskView, &str); 5] = [
    (AskView::Answer, "Answer"),
    (AskView::Source, "Source"),
    (AskView::ConceptGraph, "Graph"),
    (AskView::RetrievalPath, "Path"),
    (AskView::FollowUp, "Follow up"),
];

fn label_of(view: AskView) -> &'static str {
    VIEW_LABELS
        .iter()
        .find_map(|(shown, label)| (*shown == view).then_some(*label))
        .expect("every view has a label")
}

/// What a view draws that no other view draws: the Answer its `Results` tab, the others their
/// titles.
fn marker(view: AskView) -> (Role, &'static str) {
    match view {
        AskView::Answer => (Role::Tab, "Results"),
        AskView::Source => (Role::Label, "Source in Context"),
        AskView::ConceptGraph => (Role::Label, "Concept Graph"),
        AskView::RetrievalPath => (Role::Label, "Retrieval Path"),
        AskView::FollowUp => (Role::Label, "Follow up"),
    }
}

fn is_drawn_in(harness: &Window, view: AskView, area: Rect) -> bool {
    let (role, name) = marker(view);
    node_in(harness, role, name, area).is_some()
}

fn is_drawn(harness: &Window, view: AskView) -> bool {
    let (role, name) = marker(view);
    has(harness, role, name)
}

fn is_selected(harness: &Window, label: &str, area: Rect) -> bool {
    node_in(harness, Role::Tab, label, area)
        .unwrap_or_else(|| panic!("no tab named `{label}` is in {area:?}"))
        .accesskit_node()
        .is_selected()
        == Some(true)
}

fn assert_shows_only(harness: &Window, rects: &ShellRects, view: AskView) {
    assert_eq!(state_of(harness).ask_view, view);
    for other in AskView::ALL {
        if other == view {
            assert!(
                is_drawn_in(harness, view, rects.view(view)),
                "the {view:?} view is drawn in its room"
            );
        } else {
            assert!(!is_drawn(harness, other), "the {other:?} view is not drawn");
        }
    }
}

fn assert_items_fit_the_bar(harness: &Window, bar: Rect) {
    let mut items = Vec::new();
    for (_, label) in VIEW_LABELS {
        let item = node_in(harness, Role::Tab, label, bar)
            .unwrap_or_else(|| panic!("the bottom navigation has no item `{label}`"))
            .rect();
        fit::assert_finger_sized(label, item, size::TAB);
        items.push((label.to_owned(), item));
    }
    fit::assert_inside_and_apart(&items, bar, fit::window_rect(PHONE_WINDOW));
}

#[test]
fn a_phone_shows_one_view_of_the_ask_tab_at_a_time() {
    let mut harness = testkit::app("black-scholes", PHONE_WINDOW);
    testkit::settle(&mut harness);
    let rects = panels(PHONE_WINDOW);
    let bar = rects.bottom_nav.expect("a phone has a bottom navigation");
    assert_items_fit_the_bar(&harness, bar);
    assert_shows_only(&harness, &rects, AskView::Answer);
    testkit::save_png(&mut harness, "phone-answer");

    for (view, label) in VIEW_LABELS
        .into_iter()
        .cycle()
        .skip(1)
        .take(VIEW_LABELS.len())
    {
        click_in(&mut harness, Role::Tab, label, bar);
        assert_shows_only(&harness, &rects, view);
        assert!(is_selected(&harness, label, bar), "`{label}` is marked");
        if view == AskView::Source {
            testkit::save_png(&mut harness, "phone-source");
        }
        if view == AskView::FollowUp {
            testkit::save_png(&mut harness, "phone-follow-up");
        }
    }

    scroll_to(
        &mut harness,
        rects.answer,
        Wheel::Down,
        Role::Button,
        "Citation 1",
    );
    let citation = node_in(&harness, Role::Button, "Citation 1", rects.answer)
        .expect("a citation chip is in the Answer view");
    citation.click();
    testkit::settle(&mut harness);
    assert_shows_only(&harness, &rects, AskView::Source);
    assert!(
        harness.state().can_go_back(),
        "a phone shows the Source alone, so Back has the Answer to return to"
    );
    assert_eq!(state_of(&harness).ask.selected_result, Some(1));

    click_in(&mut harness, Role::Tab, label_of(AskView::FollowUp), bar);
    let question = state_of(&harness)
        .ask
        .answer
        .ready()
        .and_then(|answer| answer.follow_ups.first().cloned())
        .expect("the answer suggests a follow-up");
    let generation = state_of(&harness).ask.generation;
    click_in(&mut harness, Role::Button, &question, rects.follow_up);
    assert_eq!(state_of(&harness).ask.generation, generation + 1);
    assert_shows_only(&harness, &rects, AskView::Answer);

    click_in(&mut harness, Role::Tab, label_of(AskView::Source), bar);
    harness.state_mut().push(Intent::Back);
    testkit::settle(&mut harness);
    assert_shows_only(&harness, &rects, AskView::Answer);
    assert!(!state_of(&harness).can_go_back());
    assert!(!harness.state().can_go_back());
}

#[test]
fn a_tablet_shows_the_answer_beside_the_chosen_side_panel() {
    let mut harness = testkit::app("black-scholes", TABLET_WINDOW);
    testkit::settle(&mut harness);
    let rects = panels(TABLET_WINDOW);
    let tabs = rects
        .side_tabs
        .expect("a tablet has a strip over the side panel");
    let both = |harness: &Window, side: AskView| {
        assert!(
            is_drawn_in(harness, AskView::Answer, rects.answer),
            "the Answer stays on screen"
        );
        assert!(
            is_drawn_in(harness, side, rects.view(side)),
            "the {side:?} view is in the side panel"
        );
        for other in AskView::ALL {
            if other != AskView::Answer && other != side {
                assert!(!is_drawn(harness, other), "the {other:?} view is not drawn");
            }
        }
    };
    both(&harness, AskView::Source);
    for (_, label) in &VIEW_LABELS[1..] {
        let tab = node_in(&harness, Role::Tab, label, tabs)
            .unwrap_or_else(|| panic!("the strip has no tab `{label}`"))
            .rect();
        assert!(tabs.contains_rect(tab), "`{label}` {tab:?}");
    }
    testkit::save_png(&mut harness, "tablet-source");

    click_in(
        &mut harness,
        Role::Tab,
        label_of(AskView::ConceptGraph),
        tabs,
    );
    both(&harness, AskView::ConceptGraph);
    assert_eq!(state_of(&harness).side_view(), AskView::ConceptGraph);

    scroll_to(
        &mut harness,
        rects.answer,
        Wheel::Down,
        Role::Button,
        "Citation 1",
    );
    let citation = node_in(&harness, Role::Button, "Citation 1", rects.answer)
        .expect("a citation chip is in the Answer");
    citation.click();
    testkit::settle(&mut harness);
    both(&harness, AskView::Source);
    assert!(is_selected(&harness, label_of(AskView::Source), tabs));

    assert!(
        !harness.state().can_go_back(),
        "a tablet keeps the Answer on screen, so Back has nothing to change"
    );
    harness.state_mut().push(Intent::Back);
    testkit::settle(&mut harness);
    assert_eq!(
        state_of(&harness).ask_view,
        AskView::Source,
        "a Back that changes nothing on screen is dropped"
    );
    both(&harness, AskView::Source);
}
