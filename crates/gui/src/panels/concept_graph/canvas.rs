//! What the canvas does with the pointer and the keyboard: drag and wheel move the picture, a
//! click on a node or on the empty canvas sends an intent, and Tab walks the nodes.

use eframe::egui::{self, Sense, WidgetInfo, WidgetType};

use super::geometry::point;
use super::paint::{self, Drawing, NodeState, to_offset, to_point, to_pos};
use super::scene::{Picture, SCROLL_PER_DOUBLING, Scene};
use super::view::View;
use crate::contract::{ConceptGraph, GraphNode, Intent, NodeId, NodeKind};
use crate::state::AskSession;
use crate::theme::{TextRole, space};

#[derive(Debug, Clone, Copy)]
pub(super) struct Stage<'a> {
    pub canvas: egui::Rect,
    pub scene: &'a Scene,
    pub graph: &'a ConceptGraph,
    pub ask: &'a AskSession,
}

/// One frame of the canvas: it takes the input before anything is drawn, then registers the
/// nodes and paints. Between the two, the title row can read the view as the input left it.
pub(super) struct Canvas<'a> {
    stage: Stage<'a>,
    view: &'a mut View,
    background: egui::Response,
}

impl<'a> Canvas<'a> {
    /// Registers the canvas below everything else.
    pub(super) fn take_input(ui: &egui::Ui, stage: Stage<'a>, view: &'a mut View) -> Canvas<'a> {
        let background = ui.interact(
            stage.canvas,
            ui.id().with("canvas"),
            Sense::CLICK | Sense::DRAG,
        );
        let mut next = *view;
        if background.dragged() {
            next = next.panned(to_offset(background.drag_delta()));
        }
        if ui.rect_contains_pointer(stage.canvas) {
            let (zoom, scroll, pointer) = ui.input(|input| {
                (
                    input.zoom_delta(),
                    input.smooth_scroll_delta(),
                    input.pointer.hover_pos(),
                )
            });
            // The wheel is for the picture: nothing under the pointer may scroll as well.
            ui.input_mut(|input| input.smooth_scroll_delta = egui::Vec2::ZERO);
            let doublings = zoom.log2() + scroll.y / SCROLL_PER_DOUBLING;
            if let (true, Some(pointer)) = (doublings != 0.0, pointer) {
                let from_centre = to_offset(pointer - stage.canvas.center());
                next = next.zoomed(doublings, from_centre);
            }
            if scroll.x != 0.0 {
                next = next.panned(point(scroll.x, 0.0));
            }
        }
        *view = next.held(stage.scene.placed.world, stage.scene.canvas);
        Canvas {
            stage,
            view,
            background,
        }
    }

    pub(super) fn picture(&self) -> Picture<'a> {
        Picture {
            scene: self.stage.scene,
            view: *self.view,
        }
    }

    pub(super) fn reset_view(&mut self) {
        *self.view = self.stage.scene.opening;
    }

    /// Registers each node, in the order of importance, which is also the order of the Tab key.
    /// Sends the intents of the clicks, then paints with the view as it is now, so that a node
    /// that just took the keyboard is already in sight.
    pub(super) fn show_nodes(mut self, ui: &egui::Ui, intents: &mut Vec<Intent>) {
        let (responses, states) = self.register_nodes(ui, intents);
        let Stage {
            canvas,
            scene,
            graph,
            ask,
        } = self.stage;
        if self.background.clicked() && ask.focused_concept.is_some() {
            intents.push(Intent::FocusConcept(None));
        }
        let drawing = Drawing {
            canvas,
            scene,
            graph,
            view: *self.view,
        };
        paint::paint(ui, &drawing, &states);
        for (response, index) in responses.into_iter().zip(&scene.plan.nodes) {
            let node = &graph.nodes[*index];
            response.on_hover_ui(|ui| {
                ui.label(TextRole::BodyStrong.rich(&node.label));
                if let Some(detail) = &node.detail {
                    ui.label(TextRole::Small.rich(detail));
                }
            });
        }
    }

    fn register_nodes(
        &mut self,
        ui: &egui::Ui,
        intents: &mut Vec<Intent>,
    ) -> (Vec<egui::Response>, Vec<NodeState>) {
        let Stage {
            canvas,
            scene,
            graph,
            ask,
        } = self.stage;
        let centre = to_point(canvas.center());
        let registered = *self.view;
        let mut responses = Vec::with_capacity(scene.plan.nodes.len());
        let mut states = Vec::with_capacity(scene.plan.nodes.len());
        for (place, index) in scene.plan.nodes.iter().enumerate() {
            let node = &graph.nodes[*index];
            let rect = registered.rect_on_canvas(centre, scene.placed.nodes[place]);
            let rect = egui::Rect::from_min_max(to_pos(rect.min), to_pos(rect.max));
            let response = ui.interact(
                rect.intersect(canvas),
                ui.id().with(node.id),
                Sense::click(),
            );
            let is_selected = match node.id {
                NodeId::Concept(id) => ask.focused_concept == Some(id),
                NodeId::Item(number) => ask.selected_result == Some(number),
            };
            response.widget_info(|| {
                WidgetInfo::selected(WidgetType::Button, true, is_selected, name(node))
            });
            if response.gained_focus() {
                let wanted = scene.placed.nodes[place].inflate(space::XS);
                *self.view = self
                    .view
                    .showing(wanted, scene.canvas)
                    .held(scene.placed.world, scene.canvas);
            }
            if response.clicked() && !(is_selected && matches!(node.id, NodeId::Concept(_))) {
                intents.push(match node.id {
                    NodeId::Concept(id) => Intent::FocusConcept(Some(id)),
                    NodeId::Item(number) => Intent::SelectResult(number),
                });
            }
            states.push(NodeState {
                is_selected,
                is_hovered: response.hovered(),
                has_focus: response.has_focus(),
            });
            responses.push(response);
        }
        (responses, states)
    }
}

/// The accessible name of a node: what it is, and its words.
pub(super) fn name(node: &GraphNode) -> String {
    match (node.kind, node.id) {
        (NodeKind::Concept, _) => format!("Concept {}", node.label),
        (NodeKind::Related, _) => format!("Related concept {}", node.label),
        (_, NodeId::Item(number)) => format!("{}, result {number}", node.label),
        (_, NodeId::Concept(_)) => node.label.clone(),
    }
}
