//! The Source beside the Answer on a tablet, and every view in the smallest windows.

use contract::{AskView, Intent};
use eframe::egui::accesskit::Role;
use gui::app::layout::{SMALLEST_WINDOW, TABLET_WINDOW};
use gui::testkit;
use gui::widgets::Surface;

use super::{
    Body, GRAPH_TITLE, PATH_TITLE, SOURCE_TABS, SOURCE_TITLE, assert_fits, content, open_citation,
    panel_root, show_source_tab,
};
use crate::flows::{has, node_in, panels};

/// The smallest window that is laid out as a tablet, with its two panels side by side.
const MEDIUM_SMALLEST: [f32; 2] = [720.0, 560.0];

#[test]
fn a_tablet_shows_the_source_side_panel_whole() {
    let mut harness = testkit::app("black-scholes", TABLET_WINDOW);
    testkit::settle(&mut harness);
    let rects = panels(TABLET_WINDOW);
    open_citation(&mut harness, &rects, 8);
    let side = rects.source;
    testkit::save_png(&mut harness, "tablet-source");
    let panel = panel_root(&harness, SOURCE_TITLE, (Role::Tab, "Results"));
    assert_fits(&panel, content(side, Surface::Sheet), Body::Fixed);
    assert!(node_in(&harness, Role::Image, "Picture of page 5", side).is_some());
}

#[test]
fn a_cramped_window_draws_every_view_without_a_panic() {
    for (name, window) in [
        ("cramped", SMALLEST_WINDOW),
        ("medium-smallest", MEDIUM_SMALLEST),
    ] {
        let mut harness = testkit::app("black-scholes", window);
        testkit::settle(&mut harness);
        harness.state_mut().push(Intent::SelectResult(8));
        testkit::settle(&mut harness);
        let rects = panels(window);
        for (view, title) in [
            (AskView::Source, SOURCE_TITLE),
            (AskView::ConceptGraph, GRAPH_TITLE),
            (AskView::RetrievalPath, PATH_TITLE),
        ] {
            harness.state_mut().push(Intent::ShowAskView(view));
            testkit::settle(&mut harness);
            assert!(has(&harness, Role::Label, title), "{title} is drawn");
            testkit::save_png(&mut harness, &format!("{name}-{title}"));
        }
        harness
            .state_mut()
            .push(Intent::ShowAskView(AskView::Source));
        testkit::settle(&mut harness);
        let source = content(rects.source, Surface::Sheet);
        for tab in SOURCE_TABS {
            show_source_tab(&mut harness, source, tab);
        }
    }
}
