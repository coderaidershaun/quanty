//! Draws the picture with the view it has now: the links and their arrowheads and words, then
//! the nodes, then the ring of the node that has the keyboard.

use eframe::egui::epaint::{PathShape, QuadraticBezierShape};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Shape, Stroke, StrokeKind};

use super::geometry::{Point, Rect, Route, point};
use super::plan::Link;
use super::scene::{
    ARROW, CORNER, DIM, DISC, DISC_GAP, DISC_GLYPH, NodeText, PAD_LEFT, Scene, TextOpts, node_text,
    scaled_font,
};
use super::view::View;
use crate::contract::{ConceptGraph, EdgeKind, GraphNode, NodeKind};
use crate::theme::{Icon, Kind, TextRole, Tone, color, glow, radius, space, stroke};

/// How much of the node's own wash a faded node takes on, over the panel colour. The fill
/// stays solid, so a faded node still hides the links behind it.
const FADED_FILL: f32 = 0.45;
const RING_OFFSET: f32 = space::XXS;

#[derive(Debug, Clone, Copy)]
pub(super) struct Drawing<'a> {
    pub canvas: egui::Rect,
    pub scene: &'a Scene,
    pub graph: &'a ConceptGraph,
    pub view: View,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NodeState {
    /// The focused concept, or the selected result.
    pub is_selected: bool,
    pub is_hovered: bool,
    pub has_focus: bool,
}

pub(super) fn to_pos(point: Point) -> Pos2 {
    egui::pos2(point.x, point.y)
}

pub(super) fn to_point(pos: Pos2) -> Point {
    point(pos.x, pos.y)
}

pub(super) fn to_offset(vector: egui::Vec2) -> Point {
    point(vector.x, vector.y)
}

pub(super) fn paint(ui: &egui::Ui, drawing: &Drawing<'_>, states: &[NodeState]) {
    let lit = states
        .iter()
        .position(|state| state.is_hovered)
        .or_else(|| states.iter().position(|state| state.has_focus));
    let pen = Pen {
        painter: ui.painter_at(drawing.canvas),
        drawing,
        lit,
        canvas_centre: to_point(drawing.canvas.center()),
    };
    pen.links();
    pen.link_labels();
    for (place, state) in states.iter().enumerate() {
        pen.node(place, state);
    }
}

struct Pen<'a> {
    painter: Painter,
    drawing: &'a Drawing<'a>,
    /// The node under the pointer, or else the node with the keyboard.
    lit: Option<usize>,
    canvas_centre: Point,
}

struct NodeView<'a> {
    place: usize,
    rect: egui::Rect,
    node: &'a GraphNode,
    state: &'a NodeState,
    /// How much of its colour the node keeps.
    fade: f32,
    scale: f32,
}

impl Pen<'_> {
    fn scale(&self) -> f32 {
        self.drawing.view.scale()
    }

    fn at(&self, point: Point) -> Pos2 {
        to_pos(self.drawing.view.point_on_canvas(self.canvas_centre, point))
    }

    fn on_canvas(&self, rect: Rect) -> egui::Rect {
        let rect = self.drawing.view.rect_on_canvas(self.canvas_centre, rect);
        egui::Rect::from_min_max(to_pos(rect.min), to_pos(rect.max))
    }

    fn is_lit_link(&self, link: &Link) -> bool {
        self.lit.is_some_and(|lit| link.touches(lit))
    }

    fn is_in_the_light(&self, place: usize) -> bool {
        let links = &self.drawing.scene.plan.links;
        self.lit.is_none_or(|lit| {
            lit == place
                || links
                    .iter()
                    .any(|link| link.touches(lit) && link.other(lit) == place)
        })
    }

    fn link_tone(&self, link: &Link) -> Tone {
        let kind_at = |place: usize| {
            let index = self.drawing.scene.plan.nodes[place];
            self.drawing.graph.nodes[index].kind
        };
        if link.kinds.contains(&EdgeKind::Mentions) {
            Tone::Magenta
        } else if kind_at(link.a) == NodeKind::Related || kind_at(link.b) == NodeKind::Related {
            Tone::Purple
        } else {
            Tone::Blue
        }
    }

    fn links(&self) {
        let links = self.drawing.scene.plan.links.iter().enumerate();
        let of_centre = links
            .clone()
            .filter(|(_, link)| link.touches(0) && !self.is_lit_link(link));
        let of_lit = links.filter(|(_, link)| self.is_lit_link(link));
        for (index, link) in of_centre.chain(of_lit) {
            self.link(index, link);
        }
    }

    fn link(&self, index: usize, link: &Link) {
        let swatch = self.link_tone(link).swatch();
        let (colour, width) = if self.is_lit_link(link) {
            (swatch.solid, stroke::FOCUS)
        } else if self.lit.is_some() {
            (swatch.edge.gamma_multiply(DIM), stroke::EDGE)
        } else {
            (swatch.solid, stroke::EDGE)
        };
        let line = Stroke::new(width, colour);
        let route = self.drawing.scene.placed.routes[index];
        match route {
            Route::Line(from, to) => {
                self.painter
                    .line_segment([self.at(from), self.at(to)], line);
            }
            Route::Arc(from, control, to) => {
                let points = [self.at(from), self.at(control), self.at(to)];
                let curve = QuadraticBezierShape::from_points_stroke(
                    points,
                    false,
                    Color32::TRANSPARENT,
                    line,
                );
                self.painter.add(curve);
            }
            Route::Bent(points) => {
                let points = points.iter().map(|point| self.at(*point)).collect();
                self.painter.add(PathShape::line(points, line));
            }
        }
        if link.points_at_b {
            self.arrowhead(route.arrow_at_end(), colour);
        }
        if link.points_at_a {
            self.arrowhead(route.arrow_at_start(), colour);
        }
    }

    fn arrowhead(&self, along: (Point, Point), colour: Color32) {
        let (from, tip) = (self.at(along.0), self.at(along.1));
        let direction = (tip - from).normalized();
        let across = direction.rot90() * (ARROW / 2.0 * self.scale());
        let base = tip - direction * (ARROW * self.scale());
        let head = Shape::convex_polygon(
            vec![tip, base + across, base - across],
            colour,
            Stroke::NONE,
        );
        self.painter.add(head);
    }

    fn link_labels(&self) {
        let scene = self.drawing.scene;
        for (index, link) in scene.plan.links.iter().enumerate() {
            let is_lit = self.is_lit_link(link);
            let spot = scene.placed.labels[index];
            if !(is_lit || (self.lit.is_none() && spot.at_rest)) {
                continue;
            }
            let colour = if is_lit {
                color::TEXT
            } else {
                color::TEXT_SECONDARY
            };
            let font = scaled_font(TextRole::Small, self.scale());
            let galley = self
                .painter
                .layout_no_wrap(scene.link_words[index].clone(), font, colour);
            let rect = self.on_canvas(spot.rect);
            if !spot.at_rest {
                let backing = rect.expand2(egui::vec2(space::XS, 0.0));
                self.painter.rect_filled(backing, radius::SM, color::PANEL);
            }
            self.painter
                .galley(rect.center() - galley.size() / 2.0, galley, colour);
        }
    }

    fn node(&self, place: usize, state: &NodeState) {
        let drawing = self.drawing;
        let index = drawing.scene.plan.nodes[place];
        let rect = self.on_canvas(drawing.scene.placed.nodes[place]);
        if !drawing.canvas.intersects(rect) {
            return;
        }
        let view = NodeView {
            place,
            rect,
            node: &drawing.graph.nodes[index],
            state,
            fade: if self.is_in_the_light(place) {
                1.0
            } else {
                DIM
            },
            scale: self.scale(),
        };
        self.body(&view);
        self.disc_and_text(&view);
        if state.has_focus {
            let radius = self.radius(&view);
            let ring = Stroke::new(stroke::FOCUS, color::FOCUS);
            let outside = view.rect.expand(RING_OFFSET);
            self.painter
                .rect_stroke(outside, radius + RING_OFFSET, ring, StrokeKind::Outside);
        }
    }

    fn radius(&self, view: &NodeView<'_>) -> f32 {
        (CORNER * view.scale).min(view.rect.height() / 2.0)
    }

    fn body(&self, view: &NodeView<'_>) {
        let tone = Kind::from(view.node.kind).tone();
        let swatch = tone.swatch();
        let radius = self.radius(view);
        let is_centre = view.place == 0;
        if view.state.is_selected {
            self.painter.add(glow(tone).as_shape(view.rect, radius));
        }
        let fill = if view.fade < 1.0 {
            color::PANEL.lerp_to_gamma(swatch.wash, FADED_FILL)
        } else {
            swatch.wash
        };
        self.painter.rect_filled(view.rect, radius, fill);
        let (outline, width) = if is_centre || view.state.is_selected {
            (swatch.solid, stroke::EDGE)
        } else if view.state.is_hovered {
            (swatch.solid, stroke::BORDER)
        } else {
            (swatch.edge, stroke::BORDER)
        };
        let outline = Stroke::new(width, outline.gamma_multiply(view.fade));
        self.painter
            .rect_stroke(view.rect, radius, outline, StrokeKind::Inside);
    }

    fn disc_and_text(&self, view: &NodeView<'_>) {
        let kind = Kind::from(view.node.kind);
        let swatch = kind.tone().swatch();
        let scale = view.scale;
        let disc = egui::pos2(
            view.rect.left() + (PAD_LEFT + DISC / 2.0) * scale,
            view.rect.center().y,
        );
        self.painter.circle_filled(
            disc,
            DISC / 2.0 * scale,
            swatch.solid.gamma_multiply(view.fade),
        );
        self.painter.text(
            disc,
            Align2::CENTER_CENTER,
            kind.icon().glyph(),
            Icon::font(DISC_GLYPH * scale),
            swatch.on_solid.gamma_multiply(view.fade),
        );

        let opts = TextOpts {
            is_centre: view.place == 0,
            density: self.drawing.scene.density,
            scale,
            fade: view.fade,
        };
        let NodeText { first, second } = node_text(view.node, opts);
        let first = self.painter.layout_job(first);
        let second = second.map(|job| self.painter.layout_job(job));
        let height = first.size().y + second.as_ref().map_or(0.0, |galley| galley.size().y);
        let left = view.rect.left() + (PAD_LEFT + DISC + DISC_GAP) * scale;
        let top = view.rect.center().y - height / 2.0;
        let below = top + first.size().y;
        self.painter
            .galley(egui::pos2(left, top), first, color::TEXT);
        if let Some(second) = second {
            self.painter
                .galley(egui::pos2(left, below), second, color::TEXT_SECONDARY);
        }
    }
}
