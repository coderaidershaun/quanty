//! The Source, the Graph and the Path on a phone of 390 points and on one of 360.

use contract::{AskView, Intent, Panel};
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers, Rect, pos2, vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::PHONE_WINDOW;
use gui::panels::concept_graph;
use gui::testkit::{self, sample};
use gui::theme::space;
use gui::widgets::Surface;

use super::{
    Body, CONCEPTS_WARNING, GRAPH_BUTTONS, GRAPH_TITLE, LEGEND_WORDS, PATH_TITLE, SOURCE_TABS,
    SOURCE_TITLE, assert_fits, assert_tall_enough, content, figure_mark, graph_nodes,
    legend_bottom, open_citation, phone_panel, scroll_to_top, show_source_tab, show_view,
};
use crate::flows::fit::SLACK;
use crate::flows::{Window, click, has, is_enabled, node, panels, press, shared};

/// A small Android phone, which is as narrow as any screen must work.
const NARROW_PHONE: [f32; 2] = [360.0, 740.0];
/// The one concept that the sample page 5 has.
const CONCEPT: &str = "Volatility";

fn assert_the_page_is_fitted(harness: &Window) {
    assert!(has(harness, Role::Label, "100%"), "a page opens at 100%");
    assert!(
        !is_enabled(harness, Role::Button, "Fit to width"),
        "a page at 100% is fitted to the width already"
    );
    let state = shared(harness);
    let (page, target) = (state.source.page.ready(), state.source.target);
    let piece = page
        .zip(target)
        .and_then(|(page, target)| {
            page.pieces
                .iter()
                .find(|piece| Some(piece.number) == target.piece)
        })
        .expect("the figure that was cited is on the page");
    let cut = piece.cut.expect("the figure has a cut");
    let mark = figure_mark(harness);
    let picture = node(harness, Role::Image, "Picture of page 5").rect();
    let cut_width = f32::from(cut.right - cut.left);
    let width = mark.width() * 1000.0 / cut_width;
    let left = mark.left() - width * f32::from(cut.left) / 1000.0;
    let right = left + width;
    let is_inside = |edge: f32| edge >= picture.left() - SLACK && edge <= picture.right() + SLACK;
    assert!(
        is_inside(left) && is_inside(right),
        "the page spans {left}..{right}, and the picture area is {picture:?}"
    );
}

#[test]
fn the_source_fits_a_phone_on_every_tab() {
    for (name, window) in [("phone", PHONE_WINDOW), ("narrow", NARROW_PHONE)] {
        let mut harness = testkit::app("black-scholes", window);
        testkit::settle(&mut harness);
        let rects = panels(window);
        open_citation(&mut harness, &rects, 8);
        let source = content(rects.source, Surface::Sheet);

        testkit::save_png(&mut harness, &format!("{name}-source-page"));
        assert_fits(&phone_panel(&harness, SOURCE_TITLE), source, Body::Fixed);
        assert_the_page_is_fitted(&harness);
        assert_tall_enough(
            &harness,
            source,
            &[
                (Role::Button, "Open in Library"),
                (Role::Button, "Maximise"),
                (Role::ComboBox, "Media"),
                (Role::ComboBox, "Document"),
                (Role::Button, "Previous page"),
                (Role::Button, "Next page"),
                (Role::Tab, "Page"),
                (Role::Button, "Zoom out"),
                (Role::Button, "Zoom in"),
                (Role::Button, "Fit to width"),
            ],
        );

        for tab in SOURCE_TABS {
            show_source_tab(&mut harness, source, tab);
            testkit::save_png(&mut harness, &format!("{name}-source-{tab}"));
            assert_fits(&phone_panel(&harness, SOURCE_TITLE), source, Body::Scrolls);
            if tab == "Concepts" {
                assert_tall_enough(&harness, source, &[(Role::Button, CONCEPT)]);
            }
        }

        harness.state_mut().push(Intent::SelectResult(1));
        testkit::settle(&mut harness);
        scroll_to_top(&mut harness, source);
        testkit::save_png(&mut harness, &format!("{name}-source-text"));
        assert_fits(&phone_panel(&harness, SOURCE_TITLE), source, Body::Scrolls);
    }
    a_warning_about_the_concepts_stays_inside_the_source();
    a_page_that_is_gone_offers_a_try_again_that_a_finger_can_hit();
    maximising_the_source_can_be_undone_with_back();
}

/// A maximised panel takes the room of the bottom navigation too, so Back is the way out of it.
fn maximising_the_source_can_be_undone_with_back() {
    let mut harness = testkit::app("black-scholes", PHONE_WINDOW);
    testkit::settle(&mut harness);
    let rects = panels(PHONE_WINDOW);
    open_citation(&mut harness, &rects, 8);
    assert!(
        has(&harness, Role::Tab, "Answer"),
        "the bottom navigation shows"
    );

    click(&mut harness, Role::Button, "Maximise");
    assert_eq!(shared(&harness).maximised, Some(Panel::Source));
    assert!(
        !has(&harness, Role::Tab, "Answer"),
        "the bottom navigation is gone"
    );
    let page = content(rects.page, Surface::Sheet);
    let zoom_bar = node(&harness, Role::Button, "Zoom in").rect();
    assert!(
        (zoom_bar.bottom() - page.bottom()).abs() <= SLACK,
        "the Source fills the page room {page:?}, and its zoom bar ends at {}",
        zoom_bar.bottom()
    );
    assert!(
        harness.state().can_go_back(),
        "Back leaves the maximised view"
    );

    harness.state_mut().push(Intent::Back);
    testkit::settle(&mut harness);
    assert_eq!(shared(&harness).maximised, None);
    assert_eq!(shared(&harness).ask_view, AskView::Source);
    assert!(
        has(&harness, Role::Tab, "Answer"),
        "the bottom navigation is back"
    );
    assert!(has(&harness, Role::Label, SOURCE_TITLE));
}

fn a_warning_about_the_concepts_stays_inside_the_source() {
    let mut harness = testkit::app("stores-down", PHONE_WINDOW);
    testkit::settle(&mut harness);
    harness
        .state_mut()
        .push(Intent::ShowAskView(AskView::Source));
    testkit::settle(&mut harness);
    let source = content(panels(PHONE_WINDOW).source, Surface::Sheet);
    let warning = node(&harness, Role::Label, CONCEPTS_WARNING).rect();
    assert!(
        source.expand(SLACK).contains_rect(warning),
        "the warning {warning:?} is inside {source:?}"
    );
    testkit::save_png(&mut harness, "phone-source-warning");

    show_source_tab(&mut harness, source, "Concepts");
    assert_tall_enough(&harness, source, &[(Role::Button, "Try again")]);
}

fn a_page_that_is_gone_offers_a_try_again_that_a_finger_can_hit() {
    let mut harness = testkit::app("source-missing", PHONE_WINDOW);
    testkit::settle(&mut harness);
    harness
        .state_mut()
        .push(Intent::ShowAskView(AskView::Source));
    testkit::settle(&mut harness);
    let source = content(panels(PHONE_WINDOW).source, Surface::Sheet);
    assert_tall_enough(&harness, source, &[(Role::Button, "Try again")]);
}

/// Presses Tab until a node of the graph other than `other_than` has the keyboard, and returns its
/// name.
fn tab_to_a_node(harness: &mut Window, content: Rect, other_than: Option<&str>) -> String {
    for _ in 0..40 {
        press(harness, Modifiers::NONE, Key::Tab);
        let focused = harness
            .query_all_by_role(Role::Button)
            .filter(|node| node.is_focused() && content.contains(node.rect().center()))
            .filter_map(|node| node.accesskit_node().label())
            .find(|name| !GRAPH_BUTTONS.contains(&name.as_str()));
        if let Some(name) = focused.filter(|name| Some(name.as_str()) != other_than) {
            return name;
        }
    }
    panic!("Tab never reached another node of the graph");
}

#[test]
fn the_graph_and_the_path_fit_a_phone() {
    for (name, window) in [("phone", PHONE_WINDOW), ("narrow", NARROW_PHONE)] {
        let mut harness = testkit::app("black-scholes", window);
        testkit::settle(&mut harness);
        let rects = panels(window);

        show_view(&mut harness, &rects, "Graph");
        let graph = content(rects.concept_graph, Surface::Desk);
        testkit::save_png(&mut harness, &format!("{name}-graph"));
        assert_fits(&phone_panel(&harness, GRAPH_TITLE), graph, Body::Fixed);
        assert_tall_enough(
            &harness,
            graph,
            &[(Role::Button, "Maximise"), (Role::Button, "Reset view")],
        );
        if window == PHONE_WINDOW {
            let first = tab_to_a_node(&mut harness, graph, None);
            let second = tab_to_a_node(&mut harness, graph, Some(&first));
            let under = legend_bottom(&phone_panel(&harness, GRAPH_TITLE), graph);
            for focused in [first, second] {
                let rect = node(&harness, Role::Button, &focused).rect();
                assert!(
                    graph.expand(SLACK).contains_rect(rect) && rect.top() > under,
                    "the node `{focused}` {rect:?} that has the keyboard is in sight"
                );
            }
        }

        show_view(&mut harness, &rects, "Path");
        let path = content(rects.retrieval_path, Surface::Desk);
        testkit::save_png(&mut harness, &format!("{name}-path"));
        assert_fits(&phone_panel(&harness, PATH_TITLE), path, Body::Fixed);
    }
    a_legend_wider_than_the_panel_goes_on_a_second_line();
}

/// One node of each kind, each linked to the concept in the middle.
fn graph_of_every_kind() -> contract::ConceptGraph {
    use contract::{ConceptGraph, ConceptId, EdgeKind, GraphEdge, GraphNode, NodeId, NodeKind};
    let id = |number: u128| NodeId::Concept(ConceptId(uuid::Uuid::from_u128(number)));
    let graph_node = |id, kind, label: &str| GraphNode {
        id,
        kind,
        label: label.to_owned(),
        detail: None,
    };
    let edge = |from, to, kind| GraphEdge { from, to, kind };
    ConceptGraph {
        nodes: vec![
            graph_node(id(1), NodeKind::Concept, "Black–Scholes model"),
            graph_node(id(2), NodeKind::Related, "Volatility"),
            graph_node(NodeId::Item(1), NodeKind::Formula, "Formula (2.3)"),
            graph_node(NodeId::Item(3), NodeKind::Figure, "Figure 13-4"),
            graph_node(NodeId::Item(9), NodeKind::Table, "Table 1-1"),
        ],
        edges: vec![
            edge(id(1), id(2), EdgeKind::Assumes),
            edge(NodeId::Item(1), id(1), EdgeKind::Mentions),
            edge(NodeId::Item(3), id(1), EdgeKind::Mentions),
            edge(NodeId::Item(9), id(1), EdgeKind::Mentions),
        ],
    }
}

fn a_legend_wider_than_the_panel_goes_on_a_second_line() {
    let shared = testkit::answered(
        sample::search_reply(),
        graph_of_every_kind(),
        sample::answer(),
    );
    let mut local = concept_graph::Local::default();
    let panel_size = [272.0, 420.0];
    let mut harness = testkit::panel(panel_size, shared, move |ui, cx| {
        concept_graph::show(ui, &mut local, cx);
    });
    harness.run_ok();
    let gutter = space::LG;
    let panel = Rect::from_min_size(pos2(gutter, gutter), vec2(panel_size[0], panel_size[1]));
    let content = content(panel, Surface::Desk);

    let tops: Vec<f32> = LEGEND_WORDS
        .iter()
        .map(|word| {
            let found = harness
                .query_all_by_role_and_label(Role::Label, word)
                .next();
            found
                .unwrap_or_else(|| panic!("the legend has no word `{word}`"))
                .rect()
                .top()
        })
        .collect();
    assert!(
        tops[LEGEND_WORDS.len() - 1] > tops[0],
        "the legend wraps, so its last word {} is under its first {}",
        tops[LEGEND_WORDS.len() - 1],
        tops[0]
    );
    testkit::save_png(&mut harness, "graph-legend-two-lines");
    let root = harness.root();
    assert_fits(&root, content, Body::Fixed);
    let under = legend_bottom(&root, content);
    let nodes = graph_nodes(&root, content);
    assert!(!nodes.is_empty(), "the graph has nodes to look at");
    assert!(
        nodes.iter().all(|node| node.rect.top() >= under - SLACK),
        "every node is under the legend, which ends at {under}: {nodes:?}"
    );
}
