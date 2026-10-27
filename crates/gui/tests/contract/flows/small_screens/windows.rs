//! The Source beside the Answer on a tablet, and every view in the smallest windows.

use contract::{AskView, Intent};
use eframe::egui::accesskit::Role;
use gui::app::layout::{PHONE_LANDSCAPE_WINDOW, SMALLEST_WINDOW, ShellRects, TABLET_WINDOW};
use gui::testkit;
use gui::widgets::Surface;

use super::{
    Body, GRAPH_TITLE, PATH_TITLE, SOURCE_TABS, SOURCE_TITLE, assert_fits, assert_header_fits,
    content, open_citation, panel_root, show_source_tab, source_panel,
};
use crate::flows::fit::SLACK;
use crate::flows::{Window, click, drawn, has, node, node_in, panels, shared};

/// The smallest window that is laid out as a tablet, with its two panels side by side.
const MEDIUM_SMALLEST: [f32; 2] = [720.0, 560.0];
/// A window of the medium form whose side panel is a few points wider than the narrow limit, so
/// the title row of its header is as wide as the panel can give it.
const BAND_WINDOW: [f32; 2] = [980.0, 600.0];
const CHOOSE_THE_DOCUMENT: &str = "Choose the document";

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

    let mut harness = testkit::app("black-scholes", BAND_WINDOW);
    testkit::settle(&mut harness);
    let rects = panels(BAND_WINDOW);
    open_citation(&mut harness, &rects, 8);
    let panel = source_panel(&harness, &rects);
    assert_header_fits(&harness, &panel, content(rects.source, Surface::Sheet));
}

/// The part of the Source that shows the page, in points tall.
fn page_height(harness: &Window) -> f32 {
    node(harness, Role::Image, "Picture of page 5")
        .rect()
        .height()
}

fn the_pickers_fold_behind_a_toggle(harness: &mut Window, rects: &ShellRects) {
    let source = content(rects.source, Surface::Sheet);
    let pickers = [(Role::ComboBox, "Media"), (Role::ComboBox, "Document")];
    for (role, name) in pickers {
        assert!(!has(harness, role, name), "the {name} picker is folded");
    }
    click(harness, Role::Button, CHOOSE_THE_DOCUMENT);
    for (role, name) in pickers {
        let rect = node(harness, role, name).rect();
        assert!(
            source.expand(SLACK).contains_rect(rect),
            "the {name} picker {rect:?} is inside {source:?}"
        );
    }
    click(harness, Role::Button, CHOOSE_THE_DOCUMENT);
    for (role, name) in pickers {
        assert!(
            !has(harness, role, name),
            "the {name} picker is folded again"
        );
    }
}

/// The Answer pane of a phone held sideways is about 70 points under its tabs. What it says when
/// there is nothing to show, or the search failed, is all in view, and no line is cut off at its
/// foot.
fn the_answer_pane_says_all_of_its_words_when_it_is_short() {
    for scene in ["idle", "stores-down"] {
        let mut harness = testkit::app(scene, PHONE_LANDSCAPE_WINDOW);
        harness
            .state_mut()
            .push(Intent::ShowAskView(AskView::Answer));
        testkit::settle(&mut harness);
        let answer = drawn(&harness).answer;
        let words = match shared(&harness).ask.search.failure() {
            Some(failure) => ["The search failed".to_owned(), failure.hint.clone()],
            None => [
                "Ask your library".to_owned(),
                "Write a question above. The answer comes with the pages it stands on.".to_owned(),
            ],
        };
        for words in words {
            let rect = node_in(&harness, Role::Label, &words, answer)
                .unwrap_or_else(|| panic!("{scene}: `{words}` is not in the Answer pane"))
                .rect();
            assert!(
                answer.expand(SLACK).contains_rect(rect),
                "{scene}: `{words}` {rect:?} is cut off by the pane {answer:?}"
            );
        }
    }
}

#[test]
fn a_cramped_window_draws_every_view_without_a_panic() {
    // The page of the Source keeps at least this many points, so that it is worth looking at. The
    // smallest window has only 310 x 214 points inside its panel, and the folded header, the strip
    // of tabs, the zoom bar and the gaps between them take 162 of the 214, so it gets the least.
    for (name, window, page_least) in [
        ("cramped", SMALLEST_WINDOW, 48.0),
        ("medium-smallest", MEDIUM_SMALLEST, 96.0),
        ("landscape", PHONE_LANDSCAPE_WINDOW, 64.0),
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
        assert_header_fits(&harness, &source_panel(&harness, &rects), source);
        let page = page_height(&harness);
        assert!(
            page >= page_least,
            "at {window:?} the page keeps {page} points, and it needs {page_least}"
        );
        if window == SMALLEST_WINDOW {
            the_pickers_fold_behind_a_toggle(&mut harness, &rects);
        }
        for tab in SOURCE_TABS {
            show_source_tab(&mut harness, source, tab);
        }
    }
    the_answer_pane_says_all_of_its_words_when_it_is_short();
}
