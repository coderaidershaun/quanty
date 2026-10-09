//! A result chosen in any one panel is the result of all of them: the Source panel opens its
//! page, the graph marks its node, and the Answer pane brings its row into view.

use std::time::{Duration, Instant};

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::NodeT as _;
use gui::app::layout::{DEFAULT_WINDOW, MIN_WINDOW};
use gui::contract::{ConceptId, Loadable, NodeId, NodeKind, ResultItem};
use gui::state::SourceTarget;
use gui::testkit;

use super::{Wheel, Window, click, has, is_open_tab, node, panels, press, says, scroll_to, shared};

fn result(harness: &Window, number: usize) -> ResultItem {
    shared(harness)
        .ask
        .result(number)
        .unwrap_or_else(|| panic!("the search has no result {number}"))
        .clone()
}

fn graph_node_of(harness: &Window, number: usize) -> Option<String> {
    let graph = shared(harness).ask.graph.ready()?;
    let node = graph
        .nodes
        .iter()
        .find(|node| node.id == NodeId::Item(number))?;
    Some(format!("{}, result {number}", node.label))
}

fn concept_node_of(harness: &Window, concept: ConceptId) -> Option<String> {
    let graph = shared(harness).ask.graph.ready()?;
    let node = graph
        .nodes
        .iter()
        .find(|node| node.id == NodeId::Concept(concept))?;
    Some(match node.kind {
        NodeKind::Related => format!("Related concept {}", node.label),
        _ => format!("Concept {}", node.label),
    })
}

fn is_toggled(harness: &Window, role: Role, name: &str) -> bool {
    node(harness, role, name).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
}

fn assert_source_follows(harness: &Window, number: usize) {
    let found = result(harness, number);
    let shared = shared(harness);
    assert_eq!(shared.ask.selected_result, Some(number));
    assert_eq!(
        shared.source.target,
        Some(SourceTarget {
            doc: found.doc,
            page: found.page,
            piece: found.piece,
        }),
        "Source is asked for the page and the piece of result {number}"
    );
    let view = shared
        .source
        .page
        .ready()
        .unwrap_or_else(|| panic!("the page of result {number} is not ready"));
    assert_eq!((view.doc, view.page), (found.doc, found.page));
    assert!(
        has(
            harness,
            Role::Image,
            &format!("Picture of page {}", found.page)
        ),
        "the picture of the page is drawn"
    );
}

fn assert_row_is_marked(harness: &Window, number: usize) {
    let row = format!("Result {number}");
    assert!(
        has(harness, Role::Button, &row),
        "the Answer pane brought result {number} into view"
    );
    assert!(
        is_toggled(harness, Role::Button, &row),
        "the Answer pane marks the row of result {number}"
    );
}

fn wait_until(harness: &mut Window, what: &str, done: impl Fn(&Window) -> bool) {
    let started = Instant::now();
    while !done(harness) {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "gave up waiting for {what}"
        );
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn a_concept_chip_and_a_graph_node_focus_a_concept(harness: &mut Window) {
    click(harness, Role::Tab, "Concepts");
    let concepts = shared(harness)
        .source
        .concepts
        .ready()
        .cloned()
        .expect("the page of the figure has its concepts");
    let chip = concepts.first().expect("the page has a concept");
    click(harness, Role::Button, &chip.name);
    assert_eq!(shared(harness).ask.focused_concept, Some(chip.id));
    assert!(
        is_toggled(harness, Role::Button, &chip.name),
        "Source marks the concept that has the focus"
    );
    if let Some(drawn) = concept_node_of(harness, chip.id) {
        assert!(
            is_toggled(harness, Role::Button, &drawn),
            "the graph marks the concept that was focused in Source"
        );
    }
    let other: Vec<(ConceptId, String)> = shared(harness)
        .ask
        .graph
        .ready()
        .map(|graph| {
            graph
                .nodes
                .iter()
                .filter(|node| node.kind == NodeKind::Concept)
                .filter_map(|node| match node.id {
                    NodeId::Concept(id) if id != chip.id => {
                        Some((id, format!("Concept {}", node.label)))
                    }
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let (concept, name) = other.first().expect("the graph has another concept");
    click(harness, Role::Button, name);
    assert_eq!(shared(harness).ask.focused_concept, Some(*concept));
    assert!(is_toggled(harness, Role::Button, name));
}

fn every_panel_follows_a_choice(size: [f32; 2], picture: &str) {
    let mut harness = testkit::app("black-scholes", size);
    testkit::settle(&mut harness);
    let ask = &shared(&harness).ask;
    assert!(ask.search.ready().is_some() && ask.graph.ready().is_some());
    assert!(ask.answer.ready().is_some());
    let count = ask.results().len().to_string();
    assert_eq!(
        node(&harness, Role::Tab, "Results").value().as_deref(),
        Some(count.as_str()),
        "the Results tab counts the results the search found"
    );
    testkit::save_png(&mut harness, picture);

    let figure = result(&harness, 8);
    let rects = panels(size);
    scroll_to(
        &mut harness,
        rects.answer,
        Wheel::Down,
        Role::Button,
        "Citation 8",
    );
    assert!(!is_toggled(&harness, Role::Button, "Citation 8"));
    click(&mut harness, Role::Button, "Citation 8");
    assert_source_follows(&harness, 8);
    assert!(
        is_toggled(&harness, Role::Button, "Citation 8"),
        "the Answer pane marks the citation of result 8"
    );
    let on_the_page = format!(
        "{} on the page",
        figure.label.as_deref().unwrap_or_default()
    );
    assert!(has(&harness, Role::Label, &on_the_page));
    let figure_node = graph_node_of(&harness, 8).expect("the graph draws the figure");
    assert!(is_toggled(&harness, Role::Button, &figure_node));

    // The formula's page has no page pictures, so Source shows its pieces in the Page tab.
    click(&mut harness, Role::Tab, "Results");
    scroll_to(
        &mut harness,
        rects.answer,
        Wheel::Up,
        Role::Button,
        "Result 1",
    );
    click(&mut harness, Role::Button, "Result 1");
    let formula = result(&harness, 1);
    assert_eq!(shared(&harness).ask.selected_result, Some(1));
    assert_eq!(
        shared(&harness).source.target,
        Some(SourceTarget {
            doc: formula.doc,
            page: formula.page,
            piece: formula.piece,
        })
    );
    assert!(
        is_open_tab(&harness, "Page"),
        "the formula is shown in the Page tab"
    );
    let view = shared(&harness)
        .source
        .page
        .ready()
        .expect("the page of the formula is ready");
    assert_eq!((view.doc, view.page), (formula.doc, formula.page));
    assert_row_is_marked(&harness, 1);
    let formula_node = graph_node_of(&harness, 1).expect("the graph draws the formula");
    assert!(is_toggled(&harness, Role::Button, &formula_node));

    press(&mut harness, egui::Modifiers::NONE, egui::Key::J);
    assert_eq!(shared(&harness).ask.selected_result, Some(2));
    press(&mut harness, egui::Modifiers::NONE, egui::Key::K);
    assert_eq!(shared(&harness).ask.selected_result, Some(1));

    assert!(
        !has(&harness, Role::Button, "Result 8"),
        "the list starts above result 8"
    );
    click(&mut harness, Role::Button, &figure_node);
    assert_source_follows(&harness, 8);
    assert_row_is_marked(&harness, 8);
    assert!(is_toggled(&harness, Role::Button, &figure_node));

    a_concept_chip_and_a_graph_node_focus_a_concept(&mut harness);

    // The selected result clicked again shows its page again, whatever tab Source was on.
    let before = shared(&harness).cues.source_shows;
    click(&mut harness, Role::Button, "Citation 8");
    assert_eq!(shared(&harness).cues.source_shows, before + 1);
    assert!(
        is_open_tab(&harness, "Page"),
        "the target is on screen again"
    );
    assert_source_follows(&harness, 8);
}

#[test]
fn a_choice_in_one_panel_shows_in_every_panel() {
    every_panel_follows_a_choice(DEFAULT_WINDOW, "app-black-scholes");
    every_panel_follows_a_choice(MIN_WINDOW, "app-black-scholes-min");
    both_phases_of_an_ask_can_be_used_before_the_answer_is_written();
    the_header_says_what_an_ask_used();
}

/// The answer tells what the whole ask used once it lands, and an ask for results alone tells what
/// its search used.
fn the_header_says_what_an_ask_used() {
    let mut answered = testkit::app("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut answered);
    assert!(has(&answered, Role::Label, "12k tokens · ≈ $0.03"));

    let mut answering = testkit::app("answering", DEFAULT_WINDOW);
    wait_until(&mut answering, "the results", |harness| {
        shared(harness).ask.search.ready().is_some()
    });
    assert!(shared(&answering).ask.answer.is_loading());
    assert!(
        !says(&answering, "tokens"),
        "what an ask used is said once its answer lands"
    );

    let mut results_only = testkit::app("results-only", DEFAULT_WINDOW);
    testkit::settle(&mut results_only);
    assert!(has(&results_only, Role::Label, "12 tokens · under $0.01"));
}

fn both_phases_of_an_ask_can_be_used_before_the_answer_is_written() {
    let mut harness = testkit::app("answering", DEFAULT_WINDOW);
    wait_until(&mut harness, "the results", |harness| {
        shared(harness).ask.search.ready().is_some()
    });
    assert!(shared(&harness).ask.answer.is_loading());
    assert!(has(&harness, Role::Button, "Stop"));
    wait_until(&mut harness, "the row of result 1", |harness| {
        has(harness, Role::Button, "Result 1")
    });
    node(&harness, Role::Button, "Result 1").click();
    wait_until(&mut harness, "the selection", |harness| {
        shared(harness).ask.selected_result == Some(1)
    });
    assert!(
        matches!(shared(&harness).ask.answer, Loadable::Loading),
        "the answer is still on its way"
    );
    node(&harness, Role::Button, "Stop").click();
    wait_until(&mut harness, "the ask to stop", |harness| {
        !shared(harness).ask.is_running()
    });
    wait_until(&mut harness, "the Ask button", |harness| {
        has(harness, Role::Button, "Ask")
    });
}
