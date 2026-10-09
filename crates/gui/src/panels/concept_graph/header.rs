//! The title row of the panel, with the maximise button, the button that puts the picture back
//! and the badge that counts what was left out, and the row that tells what each colour stands
//! for.

use eframe::egui::{self, Align, Align2, Layout, Sense, UiBuilder, Vec2};

use super::scene::{HEADER_HEIGHT, LEGEND_DISC, LEGEND_GLYPH, LEGEND_HEIGHT, Picture};
use crate::contract::Panel;
use crate::panels::PanelCx;
use crate::theme::{Icon, Kind, TextRole, space};
use crate::widgets::{Badge, Button, section_header};

/// The areas are worked out before anything is drawn, so the canvas has its size before the
/// picture is made, and the title row still shows what the picture holds in the same frame.
#[derive(Debug, Clone, Copy)]
pub(super) struct Areas {
    header: egui::Rect,
    legend: Option<egui::Rect>,
    pub canvas: egui::Rect,
}

impl Areas {
    pub(super) fn without_legend(ui: &egui::Ui) -> Areas {
        Areas::split(ui, None)
    }

    pub(super) fn with_legend(ui: &egui::Ui) -> Areas {
        Areas::split(ui, Some(LEGEND_HEIGHT))
    }

    fn split(ui: &egui::Ui, legend_height: Option<f32>) -> Areas {
        let all = ui.available_rect_before_wrap();
        let gap = ui.spacing().item_spacing.y;
        let header = egui::Rect::from_min_size(all.min, egui::vec2(all.width(), HEADER_HEIGHT));
        let mut top = header.bottom() + gap;
        let legend = legend_height.map(|height| {
            let size = egui::vec2(all.width(), height);
            let rect = egui::Rect::from_min_size(egui::pos2(all.left(), top), size);
            top = rect.bottom() + gap;
            rect
        });
        let canvas =
            egui::Rect::from_min_max(egui::pos2(all.left(), top.min(all.bottom())), all.max);
        Areas {
            header,
            legend,
            canvas,
        }
    }

    /// Returns true when Reset view was clicked.
    pub(super) fn show_header(
        &self,
        ui: &mut egui::Ui,
        picture: Option<Picture<'_>>,
        cx: &mut PanelCx<'_>,
    ) -> bool {
        let mut reset_clicked = false;
        ui.scope_builder(UiBuilder::new().max_rect(self.header), |ui| {
            section_header(ui, "Concept Graph", |ui| {
                cx.show_maximise_toggle(ui, Panel::ConceptGraph);
                let can_reset = picture.is_some_and(|picture| picture.is_moved());
                let reset = Button::icon_only(Icon::RESET, "Reset view");
                reset_clicked = ui.add_enabled(can_reset, reset).clicked();
                let not_drawn = picture.and_then(|picture| picture.scene.not_drawn.as_ref());
                if let Some(not_drawn) = not_drawn {
                    ui.add(Badge::new(&not_drawn.badge))
                        .on_hover_text(&not_drawn.hover);
                }
            });
        });
        if let (Some(rect), Some(picture)) = (self.legend, picture) {
            let row = UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center));
            ui.scope_builder(row, |ui| legend(ui, &picture.scene.kinds));
        }
        reset_clicked
    }
}

/// The widgets have no legend entry, so it is made here from the colours and sizes of the theme.
fn legend(ui: &mut egui::Ui, kinds: &[Kind]) {
    ui.spacing_mut().item_spacing.x = space::XS;
    for (turn, kind) in kinds.iter().enumerate() {
        if turn > 0 {
            // The space that follows every item is there already, so this adds only the rest.
            ui.add_space(space::MD - space::XS);
        }
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(LEGEND_DISC), Sense::hover());
        let swatch = kind.tone().swatch();
        let painter = ui.painter();
        painter.circle_filled(rect.center(), LEGEND_DISC / 2.0, swatch.solid);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            kind.icon().glyph(),
            Icon::font(LEGEND_GLYPH),
            swatch.on_solid,
        );
        ui.label(TextRole::Small.rich(word(*kind)));
    }
}

fn word(kind: Kind) -> &'static str {
    match kind {
        Kind::Concept => "Concept",
        Kind::RelatedConcept => "Related",
        Kind::Text => "Text",
        Kind::Formula => "Formula",
        Kind::Figure => "Figure",
        Kind::Table => "Table",
    }
}
