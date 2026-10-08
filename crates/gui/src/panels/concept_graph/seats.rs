//! Where each node sits: how high the three rows stand, and the rectangle of every node in its
//! row. The links are routed afterwards, from these rectangles.

use super::geometry::{Rect, Size, point};
use super::plan::{BOTTOM, MIDDLE, Plan, TOP};

/// What the picture is made of, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Metrics {
    pub pill_height: f32,
    /// This is the room between two nodes of a row.
    pub gap: f32,
    /// This is the room on each side of a label that sits between the centre and the node
    /// beside it.
    pub label_pad: f32,
    pub band_min: f32,
    pub band_max: f32,
    pub margin: f32,
    /// How far from a corner a link may leave a node.
    pub corner: f32,
    pub label_height: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Sizes<'a> {
    /// There is one for each of `Plan::nodes`.
    pub nodes: &'a [f32],
    /// There is one for each of `Plan::links`: the width of its words.
    pub labels: &'a [f32],
    pub canvas: Size,
}

pub(super) struct Rows {
    /// The top of the top, the middle and the bottom row.
    pub top: [f32; 3],
    /// The height of the free strip between two rows.
    pub band: f32,
    /// The height of the whole picture.
    pub height: f32,
}

impl Rows {
    fn new(canvas_height: f32, metrics: &Metrics) -> Rows {
        let pills = 3.0 * metrics.pill_height;
        let band = ((canvas_height - pills - 2.0 * metrics.margin) / 2.0)
            .clamp(metrics.band_min, metrics.band_max);
        let needed = pills + 2.0 * band + 2.0 * metrics.margin;
        let height = canvas_height.max(needed);
        let first = (height - pills - 2.0 * band) / 2.0;
        let pitch = metrics.pill_height + band;
        Rows {
            top: [first, first + pitch, first + 2.0 * pitch],
            band,
            height,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

pub(super) struct Layout<'a> {
    pub plan: &'a Plan,
    pub sizes: &'a Sizes<'a>,
    pub metrics: &'a Metrics,
    pub rows: Rows,
    /// The row of each of `Plan::nodes`.
    pub row_of: Vec<usize>,
    /// The seat of each of `Plan::nodes` in its row, counted from the left.
    pub seat_of: Vec<usize>,
    pub nodes: Vec<Rect>,
}

impl<'a> Layout<'a> {
    pub(super) fn new(plan: &'a Plan, sizes: &'a Sizes<'a>, metrics: &'a Metrics) -> Layout<'a> {
        let mut row_of = vec![MIDDLE; plan.nodes.len()];
        let mut seat_of = vec![0; plan.nodes.len()];
        for (row, seats) in plan.rows.iter().enumerate() {
            for (seat, place) in seats.iter().enumerate() {
                row_of[*place] = row;
                seat_of[*place] = seat;
            }
        }
        let mut layout = Layout {
            plan,
            sizes,
            metrics,
            rows: Rows::new(sizes.canvas.h, metrics),
            row_of,
            seat_of,
            nodes: Vec::new(),
        };
        layout.nodes = layout.seat_nodes();
        layout
    }

    fn pill(&self, place: usize, left: f32) -> Rect {
        let size = Size {
            w: self.sizes.nodes[place],
            h: self.metrics.pill_height,
        };
        Rect::from_min_size(point(left, self.rows.top[self.row_of[place]]), size)
    }

    fn seat_nodes(&self) -> Vec<Rect> {
        let mut nodes = vec![Rect::default(); self.plan.nodes.len()];
        nodes[0] = self.pill(0, -self.sizes.nodes[0] / 2.0);
        for side in [Side::Right, Side::Left] {
            self.seat_middle(&mut nodes, side);
        }
        for row in [TOP, BOTTOM] {
            self.seat_packed(&mut nodes, row);
        }
        nodes
    }

    /// The width a label needs between the centre and the node beside it, when they are linked.
    fn label_gap(&self, place: usize) -> Option<f32> {
        self.plan
            .links
            .iter()
            .position(|link| link.a == 0 && link.b == place)
            .map(|index| self.sizes.labels[index] + 2.0 * self.metrics.label_pad)
    }

    /// The middle row grows out from the centre. The node next to the centre stands far enough
    /// away for the label of its link.
    fn seat_middle(&self, nodes: &mut [Rect], side: Side) {
        let middle = &self.plan.rows[MIDDLE];
        let centre_seat = self.seat_of[0];
        let places: Vec<usize> = match side {
            Side::Right => middle[centre_seat + 1..].to_vec(),
            Side::Left => middle[..centre_seat].iter().rev().copied().collect(),
        };
        let direction = if side == Side::Right { 1.0 } else { -1.0 };
        let mut edge = direction * self.sizes.nodes[0] / 2.0;
        for (step, place) in places.into_iter().enumerate() {
            let gap = match self.label_gap(place) {
                Some(wide) if step == 0 => wide,
                _ => self.metrics.gap,
            };
            let width = self.sizes.nodes[place];
            let left = match side {
                Side::Right => edge + gap,
                Side::Left => edge - gap - width,
            };
            nodes[place] = self.pill(place, left);
            edge += direction * (gap + width);
        }
    }

    /// The top and the bottom row are packed, and centred on the centre node as a group.
    fn seat_packed(&self, nodes: &mut [Rect], row: usize) {
        let seats = &self.plan.rows[row];
        let widths: f32 = seats.iter().map(|place| self.sizes.nodes[*place]).sum();
        let total = widths + self.metrics.gap * seats.len().saturating_sub(1) as f32;
        let mut left = -total / 2.0;
        for place in seats {
            nodes[*place] = self.pill(*place, left);
            left += self.sizes.nodes[*place] + self.metrics.gap;
        }
    }

    pub(super) fn world(&self) -> Rect {
        let left = self
            .nodes
            .iter()
            .map(|rect| rect.min.x)
            .fold(f32::INFINITY, f32::min)
            - self.metrics.gap;
        let right = self
            .nodes
            .iter()
            .map(|rect| rect.max.x)
            .fold(f32::NEG_INFINITY, f32::max)
            + self.metrics.gap;
        let spare = ((self.sizes.canvas.w - (right - left)) / 2.0).max(0.0);
        Rect {
            min: point(left - spare, 0.0),
            max: point(right + spare, self.rows.height),
        }
    }
}
