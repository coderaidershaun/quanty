//! The chrome that every panel starts with: the sheet or the bare desk it is drawn on, its header
//! row and a divider.

use eframe::egui::epaint::MarginF32;
use eframe::egui::{self, Color32, InnerResponse, Margin};

use crate::theme::{self, TextRole, hairline, radius, size, space, stroke};

/// What a panel is drawn on. A sheet is paper on the desk, with an outline. The desk is the
/// window itself, so its panel has no fill and no outline, and a hairline marks where it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    Sheet,
    Desk,
}

/// The frame of a panel. An egui frame has one stroke for all four sides, so the hairline across
/// the top of a desk panel is drawn here, after the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelFrame {
    surface: Surface,
    margin: Margin,
}

pub fn panel_frame(surface: Surface) -> PanelFrame {
    PanelFrame {
        surface,
        margin: Margin::same(space::LG as i8),
    }
}

impl PanelFrame {
    pub fn inner_margin(mut self, margin: impl Into<Margin>) -> Self {
        self.margin = margin.into();
        self
    }

    /// The room between the edge of the panel and its content: the margin and the outline.
    pub fn total_margin(&self) -> MarginF32 {
        // A colour does not change a margin, so none is needed here.
        self.frame(Color32::TRANSPARENT, Color32::TRANSPARENT)
            .total_margin()
    }

    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> InnerResponse<R> {
        let palette = theme::palette(ui.ctx());
        let shown = self
            .frame(palette.sheet, palette.hairline)
            .show(ui, add_contents);
        if self.surface == Surface::Desk {
            let painter = ui.painter();
            let line = hairline(painter, palette.hairline);
            let rect = shown.response.rect;
            // The line sits inside the first pixel of the rect, so it is one crisp pixel.
            painter.hline(rect.x_range(), rect.top() + line.width / 2.0, line);
        }
        shown
    }

    fn frame(&self, fill: Color32, outline: Color32) -> egui::Frame {
        let frame = egui::Frame::NONE.inner_margin(self.margin);
        match self.surface {
            Surface::Sheet => frame
                .fill(fill)
                .stroke(egui::Stroke::new(stroke::BORDER, outline))
                .corner_radius(radius::SM),
            Surface::Desk => frame,
        }
    }
}

/// The actions are laid out from right to left, so the first one that is drawn is the one at
/// the far right.
pub fn section_header<R>(
    ui: &mut egui::Ui,
    title: &str,
    add_actions: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        ui.set_min_height(size::CONTROL_SM);
        ui.label(TextRole::Heading.rich(ui, title));
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center),
            add_actions,
        )
        .inner
    })
    .inner
}

pub fn separator(ui: &mut egui::Ui) {
    let side = egui::vec2(ui.available_width(), stroke::BORDER);
    let (rect, _) = ui.allocate_exact_size(side, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let palette = theme::palette(ui.ctx());
        let painter = ui.painter();
        painter.hline(
            rect.x_range(),
            rect.center().y,
            hairline(painter, palette.hairline),
        );
    }
}
