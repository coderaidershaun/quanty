//! The title row of the panel, with the maximise and reset buttons and the count of nodes left
//! out, and the legend that tells what each colour stands for.

use eframe::egui::{self, Align, Layout, Sense, UiBuilder, Vec2};

use super::scene::{LEGEND_HEIGHT, Picture, legend_kinds};
use crate::panels::{PanelCx, PanelWidth};
use crate::theme::{Icon, Kind, TextRole, color, size, space};
use crate::widgets::{Button, ControlSize, section_header};
use contract::{ConceptGraph, Panel};

/// Below this inner width the buttons of the title row are tall enough for a finger. The panel of
/// the smallest wide window is 368 points inside and a phone's is at most 342, so a wide window is
/// never narrow.
const NARROW_BELOW: f32 = 360.0;
const LEGEND_LINE_GAP: f32 = space::XS;
const LEGEND_ITEM_GAP: f32 = space::MD;

/// The areas are worked out before anything is drawn, so the canvas has its size before the
/// picture is made, and the title row still shows what the picture holds in the same frame.
#[derive(Debug, Clone, Copy)]
pub(super) struct Areas {
    header: egui::Rect,
    control_size: ControlSize,
    legend: Option<egui::Rect>,
    pub canvas: egui::Rect,
}

impl Areas {
    pub(super) fn without_legend(ui: &egui::Ui) -> Areas {
        Areas::split(ui, None)
    }

    /// The legend takes as many lines as the kinds of `graph` need in the width of the panel.
    pub(super) fn with_legend(ui: &egui::Ui, graph: &ConceptGraph) -> Areas {
        let room = ui.available_rect_before_wrap().width();
        let lines = kinds_per_line(ui, legend_kinds(graph), room).len();
        Areas::split(ui, Some(lines))
    }

    fn split(ui: &egui::Ui, legend_lines: Option<usize>) -> Areas {
        let all = ui.available_rect_before_wrap();
        let gap = ui.spacing().item_spacing.y;
        let control_size = PanelWidth::of(ui, NARROW_BELOW).control_size();
        let header_size = egui::vec2(all.width(), control_size.height());
        let header = egui::Rect::from_min_size(all.min, header_size);
        let mut top = header.bottom() + gap;
        let legend = legend_lines.map(|lines| {
            let height =
                lines as f32 * LEGEND_HEIGHT + lines.saturating_sub(1) as f32 * LEGEND_LINE_GAP;
            let size = egui::vec2(all.width(), height);
            let rect = egui::Rect::from_min_size(egui::pos2(all.left(), top), size);
            top = rect.bottom() + gap;
            rect
        });
        let canvas =
            egui::Rect::from_min_max(egui::pos2(all.left(), top.min(all.bottom())), all.max);
        Areas {
            header,
            control_size,
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
        let size = self.control_size;
        ui.scope_builder(UiBuilder::new().max_rect(self.header), |ui| {
            // A row takes its height from the control height of the style, so that height is
            // set to the height of these buttons.
            ui.spacing_mut().interact_size.y = size.height();
            section_header(ui, "Concept Graph", |ui| {
                cx.show_maximise_toggle(ui, Panel::ConceptGraph, size);
                let can_reset = picture.is_some_and(|picture| picture.is_moved());
                let reset = Button::icon_only(Icon::RESET, "Reset view").size(size);
                reset_clicked = ui.add_enabled(can_reset, reset).clicked();
                let not_drawn = picture.and_then(|picture| picture.scene.not_drawn.as_ref());
                if let Some(not_drawn) = not_drawn {
                    ui.label(TextRole::Small.rich(&not_drawn.count))
                        .on_hover_text(&not_drawn.hover);
                }
            });
        });
        if let (Some(rect), Some(picture)) = (self.legend, picture) {
            legend(ui, rect, &picture.scene.kinds);
        }
        reset_clicked
    }
}

/// Kinds go on a line until the next one does not fit in `room`, and each line holds at least one.
/// The room kept for the legend and the legend itself are both split by this, so they agree.
fn kinds_per_line(ui: &egui::Ui, kinds: impl Iterator<Item = Kind>, room: f32) -> Vec<usize> {
    let mut lines = Vec::new();
    let mut used = 0.0;
    for kind in kinds {
        let text = TextRole::Small.galley(ui, word(kind), color::TEXT);
        let width = size::GLYPH + space::XS + text.size().x;
        match lines.last_mut() {
            Some(count) if used + LEGEND_ITEM_GAP + width <= room => {
                *count += 1;
                used += LEGEND_ITEM_GAP + width;
            }
            _ => {
                lines.push(1);
                used = width;
            }
        }
    }
    lines
}

/// The widgets have no legend entry, so it is made here from the colours and sizes of the theme.
fn legend(ui: &mut egui::Ui, rect: egui::Rect, kinds: &[Kind]) {
    let mut rest = kinds.iter();
    let mut top = rect.top();
    for count in kinds_per_line(ui, kinds.iter().copied(), rect.width()) {
        let line = egui::Rect::from_min_size(
            egui::pos2(rect.left(), top),
            egui::vec2(rect.width(), LEGEND_HEIGHT),
        );
        let row = UiBuilder::new()
            .max_rect(line)
            .layout(Layout::left_to_right(Align::Center));
        ui.scope_builder(row, |ui| {
            ui.spacing_mut().item_spacing.x = space::XS;
            for (turn, kind) in rest.by_ref().take(count).enumerate() {
                if turn > 0 {
                    // The space that follows every item is there already, so this adds only
                    // the rest.
                    ui.add_space(LEGEND_ITEM_GAP - space::XS);
                }
                let ink = kind.tone().swatch().text;
                let (glyph, _) = ui.allocate_exact_size(Vec2::splat(size::GLYPH), Sense::hover());
                kind.icon()
                    .paint(ui.painter(), glyph.center(), size::GLYPH, ink);
                ui.label(TextRole::Small.rich(word(*kind)).color(ink));
            }
        });
        top += LEGEND_HEIGHT + LEGEND_LINE_GAP;
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
        Kind::Code => "Code",
    }
}
