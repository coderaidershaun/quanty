//! The mark and the name of the app.

use std::f32::consts::{FRAC_PI_2, TAU};

use eframe::egui::{self, Pos2, Response, Shape, Stroke};

use crate::theme::{TextRole, Tone, size, space};

const FACETS: usize = 6;
const MARK_RADIUS: f32 = size::CONTROL_MD / 2.0 - 2.0;
const HOLE_RADIUS: f32 = MARK_RADIUS / 2.2;

/// The mark of the app, then its name in the title style.
pub fn logo(ui: &mut egui::Ui) -> Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::SM;
        let (_, rect) = ui.allocate_space(egui::Vec2::splat(size::CONTROL_MD));
        if ui.is_rect_visible(rect) {
            paint_mark(ui, rect.center());
        }
        ui.label(TextRole::Title.rich("quanty"));
    })
    .response
}

fn paint_mark(ui: &egui::Ui, centre: Pos2) {
    let corner = |radius: f32, index: usize| {
        let angle = TAU * index as f32 / FACETS as f32 - FRAC_PI_2;
        centre + radius * egui::vec2(angle.cos(), angle.sin())
    };
    for facet in 0..FACETS {
        // Half of the ring is magenta: the two facets at the top and the one at the right.
        let tone = if (facet + 1) % FACETS < FACETS / 2 {
            Tone::Magenta
        } else {
            Tone::Blue
        };
        let corners = vec![
            corner(MARK_RADIUS, facet),
            corner(MARK_RADIUS, facet + 1),
            corner(HOLE_RADIUS, facet + 1),
            corner(HOLE_RADIUS, facet),
        ];
        ui.painter().add(Shape::convex_polygon(
            corners,
            tone.swatch().solid,
            Stroke::NONE,
        ));
    }
}
