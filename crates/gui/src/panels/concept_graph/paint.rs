//! Draws the picture with the view it has now: the links and their arrowheads and words, then
//! the nodes, then the ring of the node that has the keyboard.

use eframe::egui::epaint::{PathShape, QuadraticBezierShape};
use eframe::egui::{self, Color32, Painter, Pos2, Shape, Stroke, StrokeKind};

use super::geometry::{Point, Rect, Route, point};
use super::plan::Link;
use super::scene::{
    ARROW, CORNER, DIM, LINK_GAP, LINK_TEXT, NodeText, PAD_LEFT, Scene, TEXT_LEFT, TextOpts,
    node_text, scaled_font,
};
use super::view::View;
use crate::contract::{ConceptGraph, GraphNode};
use crate::theme::{Kind, Tone, color, radius, size, space, stroke};

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
        let (colour, width) = if self.is_lit_link(link) {
            (color::RULE, stroke::EDGE)
        } else if self.lit.is_some() {
            (color::TEXT_SECONDARY.gamma_multiply(DIM), stroke::BORDER)
        } else {
            (color::TEXT_SECONDARY, stroke::BORDER)
        };
        let line = Stroke::new(width, colour);
        let route = self.drawing.scene.placed.routes[index];
        // The line ends at the base of an arrowhead, so it never shows past the tip.
        let at_start = LINK_GAP + if link.points_at_a { ARROW } else { 0.0 };
        let at_end = LINK_GAP + if link.points_at_b { ARROW } else { 0.0 };
        match drawn_short(route, at_start, at_end) {
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
        // SMELL: two nodes side by side can stand closer than a link with a head at each end
        // needs, so its two heads overlap a little. The layout leaves no room for the heads.
        if link.points_at_b {
            self.arrowhead(route.arrow_at_end(), colour);
        }
        if link.points_at_a {
            self.arrowhead(route.arrow_at_start(), colour);
        }
    }

    /// `along` is the line the head sits on, from the point before the end to the end itself.
    /// The tip stops `LINK_GAP` short of the end.
    fn arrowhead(&self, along: (Point, Point), colour: Color32) {
        let (from, end) = along;
        let tip = self.at(toward(end, from, LINK_GAP));
        let direction = (tip - self.at(from)).normalized();
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
            let font = scaled_font(LINK_TEXT, self.scale());
            let galley = self
                .painter
                .layout_no_wrap(scene.link_words[index].clone(), font, colour);
            let rect = self.on_canvas(spot.rect);
            if !spot.at_rest {
                let backing = rect.expand2(egui::vec2(space::XS, 0.0));
                self.painter.rect_filled(backing, radius::SM, color::DESK);
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
        self.glyph_and_text(&view);
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
        let swatch = Kind::from(view.node.kind).tone().swatch();
        let radius = self.radius(view);
        let fill = if view.fade < 1.0 {
            // A faded node mixes paper into the desk instead of turning see-through, so it still
            // hides the words of a lit link that fall under it.
            color::DESK.lerp_to_gamma(color::SHEET, DIM)
        } else if view.state.is_hovered {
            color::WELL
        } else {
            color::SHEET
        };
        self.painter.rect_filled(view.rect, radius, fill);
        let (outline, width) = if view.state.is_selected {
            (Tone::Rubric.swatch().solid, stroke::EDGE)
        } else if view.place == 0 {
            (color::RULE, stroke::EDGE)
        } else {
            (swatch.solid, stroke::BORDER)
        };
        let outline = Stroke::new(width, outline.gamma_multiply(view.fade));
        self.painter
            .rect_stroke(view.rect, radius, outline, StrokeKind::Inside);
    }

    fn glyph_and_text(&self, view: &NodeView<'_>) {
        let kind = Kind::from(view.node.kind);
        let scale = view.scale;
        let glyph = egui::pos2(
            view.rect.left() + (PAD_LEFT + size::GLYPH / 2.0) * scale,
            view.rect.center().y,
        );
        let ink = kind.tone().swatch().text.gamma_multiply(view.fade);
        kind.icon()
            .paint(&self.painter, glyph, size::GLYPH * scale, ink);

        let opts = TextOpts {
            is_centre: view.place == 0,
            density: self.drawing.scene.density,
            scale,
            fade: view.fade,
            is_selected: view.state.is_selected,
        };
        let NodeText { first, second } = node_text(view.node, opts);
        let first = self.painter.layout_job(first);
        let second = second.map(|job| self.painter.layout_job(job));
        let height = first.size().y + second.as_ref().map_or(0.0, |galley| galley.size().y);
        let left = view.rect.left() + TEXT_LEFT * scale;
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

/// The point `by` away from `from`, on the line toward `to`.
fn toward(from: Point, to: Point, by: f32) -> Point {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx.hypot(dy);
    if length == 0.0 {
        return from;
    }
    point(from.x + dx * by / length, from.y + dy * by / length)
}

/// The route with `at_start` cut off its start and `at_end` cut off its end. Only the drawing is
/// cut: the layout keeps the full route, so no label moves.
fn drawn_short(route: Route, at_start: f32, at_end: f32) -> Route {
    // When a straight route is shorter than its two cuts together, both cuts shrink by the same
    // share, so the two ends meet instead of crossing. A curved or bent route cuts its two ends
    // along two different lines.
    let share = match route {
        Route::Line(from, to) => {
            let length = (to.x - from.x).hypot(to.y - from.y);
            (length / (at_start + at_end)).min(1.0)
        }
        Route::Arc(..) | Route::Bent(..) => 1.0,
    };
    let (next, start) = route.arrow_at_start();
    let (previous, end) = route.arrow_at_end();
    let start = toward(start, next, at_start * share);
    let end = toward(end, previous, at_end * share);
    match route {
        Route::Line(..) => Route::Line(start, end),
        Route::Arc(_, control, _) => Route::Arc(start, control, end),
        Route::Bent([_, second, third, _]) => Route::Bent([start, second, third, end]),
    }
}
