//! The concepts of the answer and how they link, drawn as a graph.

mod canvas;
mod geometry;
mod header;
mod links;
mod paint;
mod place;
mod plan;
#[cfg(test)]
mod samples;
mod scene;
mod seats;
mod states;
mod view;

use eframe::egui;

use crate::contract::{ConceptGraph, NodeId};
use crate::panels::PanelCx;
use crate::state::AskSession;
use crate::theme::space;
use crate::widgets::panel_frame;
use canvas::{Canvas, Stage};
use header::Areas;
use scene::{Scene, SceneKey, Source};
use view::View;

#[derive(Debug, Default)]
pub struct Local {
    /// The `generation` of the shared state that everything below belongs to.
    seen: u64,
    view: View,
    scene: Option<Scene>,
    /// The selected result that the view was last moved to show.
    shown_selection: Option<usize>,
    /// How often a scene was built. Only a test reads it.
    #[cfg(test)]
    layouts_built: u64,
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    if local.seen != cx.shared.ask.generation {
        *local = Local {
            seen: cx.shared.ask.generation,
            ..Local::default()
        };
    }
    panel_frame().show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.spacing_mut().item_spacing = egui::vec2(space::SM, space::SM);
        if !show_graph(ui, local, cx) {
            header::show(ui, &Areas::without_legend(ui), None);
            states::show(ui, cx);
        }
    });
}

/// What `Local::refresh` needs to know: the graph, the asking, and the room.
struct Input<'a> {
    graph: &'a ConceptGraph,
    ask: &'a AskSession,
    canvas: egui::Rect,
}

/// Draws the title row, the legend and the picture of a graph that has concepts. False, with
/// nothing drawn, when there is no such graph.
fn show_graph(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) -> bool {
    let ask = &cx.shared.ask;
    let Some(graph) = ask.graph.ready().filter(|graph| plan::has_concepts(graph)) else {
        return false;
    };
    let areas = Areas::with_legend(ui);
    local.refresh(
        ui,
        &Input {
            graph,
            ask,
            canvas: areas.canvas,
        },
    );
    let Some(scene) = &local.scene else {
        return false;
    };
    let stage = Stage {
        canvas: areas.canvas,
        scene,
        graph,
        ask,
    };
    let mut canvas = Canvas::take_input(ui, stage, &mut local.view);
    if header::show(ui, &areas, Some(canvas.picture())) {
        canvas.reset_view();
    }
    canvas.show_nodes(ui, cx.intents);
    true
}

impl Local {
    /// Makes the scene for this graph and canvas when it is not made yet, and moves the view
    /// as the change asks. Brings a newly selected result into sight.
    fn refresh(&mut self, ui: &egui::Ui, input: &Input<'_>) {
        let Input { graph, ask, canvas } = *input;
        let key = SceneKey {
            generation: ask.generation,
            focus: ask.focused_concept,
            canvas: [
                canvas.width().round() as i32,
                canvas.height().round() as i32,
            ],
            pixels_per_point: ui.pixels_per_point().to_bits(),
            graph_size: [graph.nodes.len(), graph.edges.len()],
        };
        let change = Scene::refresh(&mut self.scene, ui, &Source { graph, key });
        let Some(scene) = &self.scene else {
            return;
        };
        #[cfg(test)]
        {
            self.layouts_built += u64::from(change != scene::Refresh::Unchanged);
        }
        self.view = change.follow(self.view, scene);
        if self.shown_selection != ask.selected_result {
            self.shown_selection = ask.selected_result;
            let shown = ask
                .selected_result
                .and_then(|number| scene.rect_of(graph, NodeId::Item(number)));
            if let Some(rect) = shown {
                self.view = self
                    .view
                    .showing(rect.inflate(space::XS), scene.canvas)
                    .held(scene.placed.world, scene.canvas);
            }
        }
    }
}

// SMELL: the tests below take this file over 400 lines. They all go through `show`, so they
// belong beside it. A test added here needs room made first: no file may pass 500 lines.
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use eframe::egui;
    use egui_kittest::kittest::{NodeT as _, Queryable};

    use super::samples::{
        self, DEFAULT, SMALLEST, an_empty_place, answered, click_at, drawn, drawn_nodes, is_marked,
        panel_rect, wheel, whole_width,
    };
    use crate::contract::{Intent, NodeId};
    use crate::testkit;

    #[test]
    fn a_click_focuses_a_concept_selects_an_items_result_and_an_empty_place_clears_the_focus() {
        let (mut harness, _) = drawn(DEFAULT, answered(samples::black_scholes()));
        harness.run();

        harness.get_by_label("Concept European option").click();
        harness.run();
        assert_eq!(
            harness.state().intents,
            [Intent::FocusConcept(Some(samples::concept_id(3)))]
        );
        harness.state_mut().apply_intents();
        harness.run();
        assert!(is_marked(&harness, "Concept European option"));

        // The concept that is focused already has nothing to ask for.
        harness.get_by_label("Concept European option").click();
        harness.run();
        assert!(harness.state().intents.is_empty());

        harness.get_by_label("Formula (3.17), result 1").click();
        harness.run();
        assert_eq!(harness.state().intents, [Intent::SelectResult(1)]);
        // The shared state takes only the number of a result that the search found, and the
        // search of the test kit knows nothing of this graph. So the selection is set here, as
        // the shared state would set it.
        let host = harness.state_mut();
        host.intents.clear();
        host.shared.ask.selected_result = Some(1);
        harness.run();
        assert!(is_marked(&harness, "Formula (3.17), result 1"));

        click_at(&mut harness, an_empty_place());
        assert_eq!(harness.state().intents, [Intent::FocusConcept(None)]);
        harness.state_mut().apply_intents();
        harness.run();
        assert!(!is_marked(&harness, "Concept European option"));
    }

    #[test]
    fn the_wireframe_graph_is_all_in_sight_at_the_default_size_and_rests() {
        let (mut smallest, _) = drawn(SMALLEST, answered(samples::black_scholes()));
        smallest.run();
        testkit::save_png(&mut smallest, "concept-graph-ready-smallest");

        let graph = samples::black_scholes();
        let (mut harness, local) = drawn(DEFAULT, answered(graph.clone()));
        harness.run();
        testkit::save_png(&mut harness, "concept-graph-ready");
        let panel = panel_rect(DEFAULT);
        let nodes = drawn_nodes(&local, &graph);
        for name in [
            "Concept Black–Scholes model",
            "Concept Geometric Brownian motion",
            "Concept European option",
            "Related concept Risk-neutral measure",
            "Related concept Itô's lemma",
            "Formula (3.17), result 1",
            "Figure 3.1, result 8",
        ] {
            let rect = harness.get_by_label(name).rect();
            let place = nodes
                .iter()
                .position(|node| super::canvas::name(node) == name);
            let whole = whole_width(&local, place.expect("the node is drawn"));
            assert!(
                panel.contains_rect(rect) && rect.width() >= whole - 1.0,
                "{name} is at {rect:?}: cut, or outside {panel:?}"
            );
        }

        let local = local.borrow();
        let scene = local.scene.as_ref().expect("the graph has a scene");
        let resting = scene.placed.labels.iter().filter(|label| label.at_rest);
        assert_eq!(resting.count(), 6, "every link of the centre names itself");

        let delay = harness.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        assert_eq!(
            delay,
            Duration::MAX,
            "a drawn graph at rest asks for no repaint"
        );
    }

    #[test]
    fn selection_from_other_panels_is_marked_and_the_keyboard_reaches_every_node() {
        let mut shared = answered(samples::black_scholes());
        shared.ask.focused_concept = Some(samples::concept_id(3));
        shared.ask.selected_result = Some(1);
        let (mut harness, _) = drawn(DEFAULT, shared);
        harness.run();
        testkit::save_png(&mut harness, "concept-graph-focused");
        assert!(is_marked(&harness, "Concept European option"));
        assert!(is_marked(&harness, "Formula (3.17), result 1"));
        assert!(!is_marked(&harness, "Concept Black–Scholes model"));
        assert!(!is_marked(&harness, "Figure 3.1, result 8"));

        // A graph too big to see whole: Tab walks the nodes in order of importance, and each
        // one it reaches is brought into sight, whole.
        let graph = samples::dense(30);
        let (mut harness, local) = drawn(DEFAULT, answered(graph.clone()));
        harness.run();
        let nodes = drawn_nodes(&local, &graph);
        let panel = panel_rect(DEFAULT);
        for (place, node) in nodes.iter().enumerate() {
            harness.key_press(egui::Key::Tab);
            harness.run();
            let focused = harness
                .get_all_by(|node| node.is_focused())
                .next()
                .expect("something has the keyboard");
            let name = super::canvas::name(node);
            assert_eq!(
                focused.accesskit_node().label().as_deref(),
                Some(name.as_str())
            );
            let rect = focused.rect();
            assert!(
                panel.contains_rect(rect),
                "{name} is at {rect:?}, outside {panel:?}"
            );
            assert!(
                rect.width() >= whole_width(&local, place) - 1.0,
                "{name} is cut: {rect:?}"
            );
        }

        // The Tab key then goes round to `Reset view`, which the moves have made live, then to the
        // centre and the next node, and Enter does what a click does.
        harness.key_press(egui::Key::Tab);
        harness.run();
        assert!(harness.get_by_label("Reset view").is_focused());
        harness.key_press(egui::Key::Tab);
        harness.key_press(egui::Key::Tab);
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        let wanted = match nodes[1].id {
            NodeId::Concept(id) => Intent::FocusConcept(Some(id)),
            NodeId::Item(number) => Intent::SelectResult(number),
        };
        assert_eq!(harness.state().intents, [wanted]);

        // A result that another panel selects while it is out of sight is brought into sight.
        let hidden = nodes.iter().enumerate().find_map(|(place, node)| {
            let NodeId::Item(number) = node.id else {
                return None;
            };
            let name = super::canvas::name(node);
            let seen = harness.get_by_label(&name).rect().width();
            (seen < whole_width(&local, place) - 1.0).then_some((place, number, name))
        });
        let (place, number, name) = hidden.expect("some result is out of sight");
        harness.state_mut().shared.ask.selected_result = Some(number);
        harness.run();
        let rect = harness.get_by_label(&name).rect();
        assert!(is_marked(&harness, &name));
        assert!(
            panel.contains_rect(rect) && rect.width() >= whole_width(&local, place) - 1.0,
            "{name} was selected and is at {rect:?}"
        );
    }

    #[test]
    fn drag_pans_scroll_and_pinch_zoom_within_limits_and_reset_view_undoes_them() {
        let graph = samples::dense(30);
        let (mut harness, local) = drawn(DEFAULT, answered(graph.clone()));
        harness.run();
        let opening = local.borrow().view;
        let centre_name = super::canvas::name(drawn_nodes(&local, &graph)[0]);
        let reset_is_off = |harness: &egui_kittest::Harness<'_, testkit::Host>| {
            harness
                .get_by_label("Reset view")
                .accesskit_node()
                .is_disabled()
        };
        assert!(
            reset_is_off(&harness),
            "an untouched view has nothing to reset"
        );
        assert_eq!(opening.step(), 0, "a graph this size opens at full size");

        // A drag that starts on a node moves the picture, and does not click the node.
        let on_a_node = harness.get_by_label(&centre_name).rect().center();
        let to = on_a_node + egui::vec2(-90.0, 0.0);
        harness.drag_at(on_a_node);
        harness.step();
        harness.hover_at(to);
        harness.step();
        assert!(
            !reset_is_off(&harness),
            "the button is live in the frame that moves the picture"
        );
        harness.drop_at(to);
        harness.run_ok();
        let dragged = local.borrow().view;
        assert!(
            dragged.at.x > opening.at.x + 80.0,
            "{dragged:?} after {opening:?}"
        );
        assert!(harness.state().intents.is_empty(), "a drag is not a click");
        assert!(!reset_is_off(&harness), "a moved view can be put back");

        // Scrolling sideways moves it, and scrolling up or down zooms about the pointer, but
        // never beyond 50 % and 200 %.
        harness.hover_at(harness.get_by_label(&centre_name).rect().center());
        harness.run_ok();
        wheel(&harness, egui::vec2(60.0, 0.0));
        harness.run_ok();
        assert!(local.borrow().view.at.x < dragged.at.x);
        wheel(&harness, egui::vec2(0.0, -120.0));
        harness.run_ok();
        let zoomed_out = local.borrow().view.step();
        assert!(zoomed_out < 0, "a wheel down zooms out: step {zoomed_out}");
        harness.event(egui::Event::Zoom(1.5));
        harness.run_ok();
        assert!(
            local.borrow().view.step() > zoomed_out,
            "a pinch out zooms in"
        );
        for _ in 0..20 {
            wheel(&harness, egui::vec2(0.0, 240.0));
        }
        harness.run_ok();
        assert_eq!(local.borrow().view.step(), 4);
        for _ in 0..40 {
            wheel(&harness, egui::vec2(0.0, -240.0));
        }
        harness.run_ok();
        let smallest = local.borrow().view;
        assert_eq!(smallest.step(), -4);
        let local_ref = local.borrow();
        let world = local_ref
            .scene
            .as_ref()
            .expect("the graph has a scene")
            .placed
            .world;
        assert!(world.min.x <= smallest.at.x && smallest.at.x <= world.max.x);
        drop(local_ref);

        // `Reset view` gives back the opening view, exactly, and nothing built a second layout.
        harness.get_by_label("Reset view").click();
        harness.run_ok();
        assert_eq!(local.borrow().view, opening);
        assert!(reset_is_off(&harness));
        assert_eq!(
            local.borrow().layouts_built,
            1,
            "pan and zoom never lay out again"
        );
    }

    #[test]
    fn a_dense_graph_opens_on_its_centre_counts_what_it_leaves_out_and_lights_what_is_pointed_at() {
        let graph = samples::dense(45);
        let (mut harness, local) = drawn(DEFAULT, answered(graph.clone()));
        // The badge is there in the frame that made the picture, not a frame later.
        harness.get_by_label("+9 not drawn");
        harness.run();
        testkit::save_png(&mut harness, "concept-graph-dense");
        for kind in ["Concept", "Related", "Formula", "Figure", "Table"] {
            harness.get_by_label(kind);
        }

        // It opens at full size, with the centre at the middle of the canvas.
        let panel = panel_rect(DEFAULT);
        let nodes = drawn_nodes(&local, &graph);
        assert_eq!(nodes.len(), 36);
        assert_eq!(local.borrow().view.step(), 0);
        let centre_name = super::canvas::name(nodes[0]);
        let centre = harness.get_by_label(&centre_name).rect().center();
        assert!(
            (centre.x - panel.center().x).abs() < 1.0,
            "{centre:?} in {panel:?}"
        );

        // Pointing at a node, one that is in sight, shows the words of that node.
        let in_sight = nodes.iter().enumerate().skip(1).find_map(|(place, node)| {
            let rect = harness.get_by_label(&super::canvas::name(node)).rect();
            let is_whole = rect.width() >= whole_width(&local, place) - 1.0;
            (is_whole && panel.contains_rect(rect)).then_some((rect, node))
        });
        let (rect, node) = in_sight.expect("some node besides the centre is in sight");
        let words = node.detail.as_deref().expect("every node here has words");
        harness.hover_at(rect.center());
        harness.run_ok();
        harness.run_ok();
        samples::save_png_with_pointer(&mut harness, "concept-graph-dense-hover");
        // SMELL: this checks only the words that a pointed-at node shows. That the other nodes
        // fade is checked by no test: it is seen only in the picture written above.
        assert!(
            harness.query_all_by_label_contains(words).next().is_some(),
            "the node shows its words when it is pointed at"
        );
    }
}
