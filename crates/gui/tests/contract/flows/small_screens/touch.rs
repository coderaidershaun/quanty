//! A finger on the page and on the graph: a pinch zooms, a drag pans, and a pinch is no click.

use contract::AskView;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, Rect, TouchDeviceId, TouchId};
use eframe::egui::{TouchPhase, Vec2, pos2, vec2};
use gui::app::layout::PHONE_WINDOW;
use gui::testkit;
use gui::widgets::Surface;

use super::{
    GRAPH_TITLE, content, figure_mark, graph_nodes, legend_bottom, open_citation, phone_panel,
    show_view,
};
use crate::flows::fit::SLACK;
use crate::flows::{Window, click_in, has, is_enabled, node, panels, shared};

const FRAME_SECONDS: f64 = 1.0 / 60.0;
const TOUCH_STEPS: usize = 8;
/// How far a dragged page or node may end from where the finger took it.
const MOVE_SLACK: f32 = 6.0;

/// One frame that carries `events` together, a sixtieth of a second after the last one, as a
/// phone delivers a touch. The harness's own frames are a quarter of a second apart, which makes
/// every gesture too slow to count as a click, so a pinch that clicks would go unseen.
fn frame(harness: &mut Window, events: Vec<Event>) {
    let now = harness.ctx.input(|input| input.time);
    harness.input_mut().time = Some(now + FRAME_SECONDS);
    harness.input_mut().events.extend(events);
    harness.step();
}

fn touches(phase: TouchPhase, fingers: &[Pos2]) -> Vec<Event> {
    fingers
        .iter()
        .enumerate()
        .map(|(id, pos)| Event::Touch {
            device_id: TouchDeviceId(0),
            id: TouchId(id as u64),
            phase,
            pos: *pos,
            force: None,
        })
        .collect()
}

fn pointer_button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Sends what the browser sends for fingers on the screen: the first finger is also the pointer,
/// and every finger is a touch. `at` gives the places of the fingers a share of the way through
/// the gesture.
fn touch(harness: &mut Window, at: impl Fn(f32) -> Vec<Pos2>) {
    let start = at(0.0);
    let down = [
        vec![pointer_button(start[0], true)],
        touches(TouchPhase::Start, &start),
    ];
    frame(harness, down.concat());
    for step in 1..=TOUCH_STEPS {
        let fingers = at(step as f32 / TOUCH_STEPS as f32);
        let moved = [
            vec![Event::PointerMoved(fingers[0])],
            touches(TouchPhase::Move, &fingers),
        ];
        frame(harness, moved.concat());
    }
    let end = at(1.0);
    let up = [
        vec![pointer_button(end[0], false), Event::PointerGone],
        touches(TouchPhase::End, &end),
    ];
    frame(harness, up.concat());
    harness.input_mut().time = None;
    testkit::settle(harness);
}

fn pinch(harness: &mut Window, centre: Pos2, from_gap: f32, to_gap: f32) {
    touch(harness, |share| {
        let half = (from_gap + (to_gap - from_gap) * share) / 2.0;
        vec![centre - vec2(half, 0.0), centre + vec2(half, 0.0)]
    });
}

fn drag(harness: &mut Window, from: Pos2, by: Vec2) {
    touch(harness, |share| vec![from + by * share]);
}

#[test]
fn a_pinch_zooms_and_a_drag_pans_the_page_and_the_graph() {
    let mut harness = testkit::app("black-scholes", PHONE_WINDOW);
    testkit::settle(&mut harness);
    let rects = panels(PHONE_WINDOW);
    open_citation(&mut harness, &rects, 8);
    a_finger_moves_the_page(&mut harness);

    click_in(&mut harness, Role::Button, "Next page", rects.source);
    assert_eq!(
        shared(&harness).source.page.ready().map(|view| view.page),
        Some(6)
    );
    assert!(
        has(&harness, Role::Label, "100%"),
        "in a narrow panel every page that arrives is fitted to the width"
    );

    show_view(&mut harness, &rects, "Graph");
    let graph = content(rects.concept_graph, Surface::Desk);
    a_finger_moves_the_graph(&mut harness, graph);
    a_pinch_does_not_click_the_node_under_a_finger(&mut harness, graph);
}

fn a_finger_moves_the_page(harness: &mut Window) {
    assert!(has(harness, Role::Label, "100%"));
    let picture = node(harness, Role::Image, "Picture of page 5").rect();
    pinch(harness, picture.center(), 80.0, 160.0);
    assert!(
        !has(harness, Role::Label, "100%") && is_enabled(harness, Role::Button, "Fit to width"),
        "two fingers spreading zoom the page in"
    );

    let before = figure_mark(harness).bottom();
    drag(harness, picture.center(), vec2(0.0, -60.0));
    let moved = figure_mark(harness).bottom() - before;
    assert!(
        (moved + 60.0).abs() <= MOVE_SLACK,
        "one finger dragging up 60 points moved the page by {moved}"
    );
}

fn a_finger_moves_the_graph(harness: &mut Window, graph: Rect) {
    let under = legend_bottom(&phone_panel(harness, GRAPH_TITLE), graph);
    let canvas = Rect::from_min_max(pos2(graph.left(), under), graph.max);
    let centre_node = "Concept Black–Scholes model";
    let width = |harness: &Window| node(harness, Role::Button, centre_node).rect().width();
    let narrow = width(harness);
    pinch(harness, canvas.center(), 60.0, 140.0);
    assert!(
        width(harness) >= narrow * 1.15,
        "two fingers spreading zoom the graph in: {narrow} then {}",
        width(harness)
    );

    let before = graph_nodes(&phone_panel(harness, GRAPH_TITLE), graph);
    let by = vec2(40.0, 0.0);
    drag(harness, canvas.center(), by);
    let after = graph_nodes(&phone_panel(harness, GRAPH_TITLE), graph);
    let moved_node = before.iter().find_map(|was| {
        let now = after.iter().find(|now| now.name == was.name)?;
        let is_whole = (now.rect.size() - was.rect.size()).abs().max_elem() <= SLACK;
        is_whole.then(|| now.rect.center() - was.rect.center())
    });
    let moved_node = moved_node.expect("a node of the graph stays in sight and whole");
    assert!(
        (moved_node - by).abs().max_elem() <= MOVE_SLACK,
        "one finger dragging {by:?} moved a node by {moved_node:?}"
    );
    assert!(is_enabled(harness, Role::Button, "Reset view"));
}

fn a_pinch_does_not_click_the_node_under_a_finger(harness: &mut Window, graph: Rect) {
    click_in(harness, Role::Button, "Reset view", graph);
    let selected = shared(harness).ask.selected_result;
    let result = graph_nodes(&phone_panel(harness, GRAPH_TITLE), graph)
        .into_iter()
        .find(|node| node.name.contains(", result "))
        .expect("a result node is on the canvas");
    let resting = result.rect.center();
    touch(harness, |share| {
        vec![resting, resting + vec2(60.0 + 80.0 * share, 0.0)]
    });
    assert_eq!(shared(harness).ask_view, AskView::ConceptGraph);
    assert_eq!(shared(harness).ask.selected_result, selected);
}
