//! The app on phones, a tablet and in cramped windows: every control lies inside its panel and
//! apart from the others, the main controls are tall enough for a finger, and a finger moves the
//! page and the graph. The Answer and Follow up views are in `answer`, the Ask bar in `ask_bar`,
//! the views of the Ask tab and Back in `ask_views`, the Ingest and Datasets forms in `forms`, the
//! Source, Graph and Path in `source`, the finger gestures in `touch`, and the smallest windows in
//! `windows`.

mod answer;
mod ask_bar;
mod ask_views;
mod forms;
mod source;
mod touch;
mod windows;

use contract::AskView;
use eframe::egui::accesskit::Role;
use eframe::egui::{self, Rect};
use egui_kittest::Node;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::{Form, ShellRects};
use gui::theme::{size, space};
use gui::widgets::{Surface, panel_frame};

use super::fit::{self, SLACK};
use super::{Wheel, Window, click_in, has, node, scroll_to, shared};

const CONTROL_ROLES: [Role; 8] = [
    Role::Button,
    Role::ComboBox,
    Role::Tab,
    Role::Slider,
    Role::Image,
    Role::Label,
    Role::TextInput,
    Role::CheckBox,
];
const LEGEND_WORDS: [&str; 5] = ["Concept", "Related", "Formula", "Figure", "Table"];
/// The buttons of the graph's title row, which are not nodes of the graph.
const GRAPH_BUTTONS: [&str; 2] = ["Maximise", "Reset view"];
const SOURCE_TABS: [&str; 5] = ["Figures", "Formulas", "Tables", "Code", "Concepts"];
const SOURCE_TITLE: &str = "Source in Context";
const GRAPH_TITLE: &str = "Concept Graph";
const PATH_TITLE: &str = "Retrieval Path";
const CONCEPTS_WARNING: &str = "The concepts could not be read";

#[derive(Debug, Clone)]
struct Control {
    role: Role,
    name: String,
    rect: Rect,
    is_selected: bool,
}

/// Whether the part of a panel that holds the controls shows all of them, or scrolls down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Body {
    Fixed,
    Scrolls,
}

/// A panel's room for its controls: the panel less the margin and the outline of its frame.
fn content(panel: Rect, surface: Surface) -> Rect {
    let margin = panel_frame(surface).total_margin();
    Rect::from_min_max(
        panel.min + margin.left_top(),
        panel.max - margin.right_bottom(),
    )
}

/// What a node is called: a label is named by its text, and any other node by its own name.
fn name_of(node: &Node<'_>) -> String {
    let found = node.accesskit_node();
    found.label().or_else(|| found.value()).unwrap_or_default()
}

fn ancestors<'a>(node: &Node<'a>) -> Vec<Node<'a>> {
    let mut chain = Vec::new();
    let mut at = node.parent();
    while let Some(found) = at {
        at = found.parent();
        chain.push(found);
    }
    chain
}

/// The highest container of the title that does not also hold `rival`, a node of another part of
/// the window. The tree follows the containers, so this holds every control of the panel and no
/// other, even one that a scroll area cut off over another panel.
fn panel_root<'a>(harness: &'a Window, title: &'a str, rival: (Role, &'a str)) -> Node<'a> {
    let shared: Vec<_> = ancestors(&node(harness, rival.0, rival.1))
        .iter()
        .map(|found| found.accesskit_node().id())
        .collect();
    let mut root = node(harness, Role::Label, title);
    while let Some(parent) = root.parent() {
        if shared.contains(&parent.accesskit_node().id()) {
            return root;
        }
        root = parent;
    }
    panic!("`{title}` and `{}` are in no common container", rival.1)
}

/// The Source panel, in a window whose parts are at `rects`: a phone has the bottom navigation
/// beside it, and a tablet has the Answer.
fn source_panel<'a>(harness: &'a Window, rects: &ShellRects) -> Node<'a> {
    let rival = match rects.form {
        Form::Compact => (Role::Tab, "Answer"),
        Form::Medium | Form::Wide => (Role::Tab, "Results"),
    };
    panel_root(harness, SOURCE_TITLE, rival)
}

/// The panel of the one view that a phone draws. The bottom navigation, with its Answer tab, is
/// another part of the window.
fn phone_panel<'a>(harness: &'a Window, title: &'a str) -> Node<'a> {
    panel_root(harness, title, (Role::Tab, "Answer"))
}

fn controls(root: &Node<'_>) -> Vec<Control> {
    root.children_recursive()
        .filter(|node| CONTROL_ROLES.contains(&node.accesskit_node().role()))
        .filter(|node| node.accesskit_node().bounding_box().is_some())
        .map(|node| Control {
            role: node.accesskit_node().role(),
            name: name_of(&node),
            rect: node.rect(),
            is_selected: node.accesskit_node().is_selected() == Some(true),
        })
        .filter(|control| control.rect.is_positive())
        .collect()
}

fn sticks_out(control: &Control, room: Rect, body: Body) -> bool {
    let rect = control.rect;
    let is_inside_sideways = rect.left() >= room.left() && rect.right() <= room.right();
    let is_inside_above = rect.top() >= room.top();
    let is_inside_below = rect.bottom() <= room.bottom();
    match (control.role, body) {
        // A strip of tabs scrolls sideways, but the open tab is always in view.
        (Role::Tab, _) => {
            !(is_inside_above && is_inside_below && (is_inside_sideways || !control.is_selected))
        }
        (_, Body::Scrolls) => !(is_inside_sideways && is_inside_above),
        (_, Body::Fixed) => !(is_inside_sideways && is_inside_above && is_inside_below),
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    let is_nested = a.expand(SLACK).contains_rect(b) || b.expand(SLACK).contains_rect(a);
    fit::overlaps(a, b) && !is_nested
}

/// Every control under `root` lies inside `content`, and no two of them overlap. A control inside
/// another, such as a citation chip in its paragraph, is not an overlap.
fn assert_fits(root: &Node<'_>, content: Rect, body: Body) {
    assert_controls_fit(controls(root), content, body);
}

fn assert_controls_fit(found: Vec<Control>, content: Rect, body: Body) {
    // A tree that stops nesting the controls of a panel in a container would leave nothing to
    // check, and the test would pass without looking at the panel.
    assert!(
        !found.is_empty(),
        "no control was found to fit in {content:?}"
    );
    let room = content.expand(SLACK);
    let mut faults: Vec<String> = found
        .iter()
        .filter(|control| sticks_out(control, room, body))
        .map(|control| format!("{control:?} sticks out of {content:?}"))
        .collect();
    let whole: Vec<&Control> = found
        .iter()
        .filter(|control| room.contains_rect(control.rect))
        .collect();
    for (place, a) in whole.iter().enumerate() {
        for b in &whole[place + 1..] {
            if overlaps(a.rect, b.rect) {
                faults.push(format!("{a:?} overlaps {b:?}"));
            }
        }
    }
    assert!(
        faults.is_empty(),
        "{} controls, and these do not fit:\n{}",
        found.len(),
        faults.join("\n")
    );
}

/// The controls above the strip of tabs are the header's: they lie inside the panel and apart.
fn assert_header_fits(harness: &Window, root: &Node<'_>, content: Rect) {
    let strip_top = node(harness, Role::Tab, "Page").rect().top();
    let header = controls(root)
        .into_iter()
        .filter(|control| control.rect.bottom() <= strip_top + SLACK)
        .collect();
    assert_controls_fit(header, content, Body::Fixed);
}

fn assert_tall_enough(harness: &Window, content: Rect, names: &[(Role, &str)]) {
    for (role, name) in names {
        let found = harness
            .query_all_by_role_and_label(*role, name)
            .find(|node| content.contains(node.rect().center()))
            .unwrap_or_else(|| panic!("no {role:?} named `{name}` is in {content:?}"));
        fit::assert_finger_sized(name, found.rect(), size::CONTROL_MD);
    }
}

fn open_citation(harness: &mut Window, rects: &ShellRects, number: usize) {
    let name = format!("Citation {number}");
    scroll_to(harness, rects.answer, Wheel::Down, Role::Button, &name);
    click_in(harness, Role::Button, &name, rects.answer);
    assert_eq!(shared(harness).ask_view, AskView::Source);
}

fn show_view(harness: &mut Window, rects: &ShellRects, label: &str) {
    let bar = rects.bottom_nav.expect("a phone has a bottom navigation");
    click_in(harness, Role::Tab, label, bar);
}

/// A strip of tabs that overflows scrolls under the wheel, so the wheel is turned until the tab
/// is wholly in the part of the row that is in sight.
fn show_source_tab(harness: &mut Window, content: Rect, label: &str) {
    let row = node(harness, Role::Tab, "Page").rect();
    let tab = harness
        .query_all_by_role_and_label(Role::Tab, label)
        .map(|found| found.rect())
        .find(|rect| row.y_range().contains(rect.center().y))
        .unwrap_or_else(|| panic!("the Source has no tab `{label}`"));
    let warning_room = if has(harness, Role::Label, CONCEPTS_WARNING) {
        space::XS + size::GLYPH
    } else {
        0.0
    };
    let room = Rect::from_x_y_ranges(
        content.left() - SLACK..=content.right() - warning_room + SLACK,
        row.top()..=row.bottom(),
    );
    let wheel = if tab.center().x < room.center().x {
        Wheel::Up
    } else {
        Wheel::Down
    };
    scroll_to(harness, room, wheel, Role::Tab, label);
    click_in(harness, Role::Tab, label, room);
}

/// A list that was scrolled to bring a piece into view is turned back to its top, so that no row
/// is above the panel, where a scroll area cuts it off but the tree of the window still has it.
fn scroll_to_top(harness: &mut Window, over: Rect) {
    for _ in 0..20 {
        harness.hover_at(over.center());
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 240.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_ok();
    }
}

fn legend_bottom(root: &Node<'_>, content: Rect) -> f32 {
    controls(root)
        .iter()
        .filter(|control| control.role == Role::Label && LEGEND_WORDS.contains(&&*control.name))
        .filter(|control| content.contains(control.rect.center()))
        .map(|control| control.rect.bottom())
        .fold(content.top(), f32::max)
}

/// The nodes of the graph are the buttons of the canvas, which is under the legend.
fn graph_nodes(root: &Node<'_>, content: Rect) -> Vec<Control> {
    let top = legend_bottom(root, content);
    controls(root)
        .into_iter()
        .filter(|control| control.role == Role::Button && control.rect.center().y > top)
        .filter(|control| !GRAPH_BUTTONS.contains(&control.name.as_str()))
        .collect()
}

/// The frame round the cited figure on the page picture.
fn figure_mark(harness: &Window) -> Rect {
    harness
        .query_all_by_role(Role::Label)
        .find(|node| name_of(node).ends_with(" on the page"))
        .expect("the cited figure is framed on the page")
        .rect()
}
