//! The concepts of the answer and how they link, drawn as a graph.

mod canvas;
mod geometry;
mod header;
mod links;
mod paint;
mod place;
mod plan;
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

struct Input<'a> {
    graph: &'a ConceptGraph,
    ask: &'a AskSession,
    canvas: egui::Rect,
}

/// Returns false, with nothing drawn, when there is no graph that has concepts.
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
