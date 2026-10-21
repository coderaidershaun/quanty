//! A row of a list: its text at the left, cut with "…" when it is too long, a glyph before it, a
//! count at the right end, and a fill while it is chosen or under the pointer.

use eframe::egui::{self, Response, Sense, WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2};

use super::look::{Look, focus_ring};
use crate::theme::{Icon, TextRole, color, radius, size, space, stroke};

pub struct ListRow<'a> {
    label: &'a str,
    glyph: Option<Icon>,
    count: Option<usize>,
    indent: f32,
    is_selected: bool,
    role: TextRole,
    forced: Option<Look>,
}

impl<'a> ListRow<'a> {
    pub fn new(label: &'a str) -> Self {
        ListRow {
            label,
            glyph: None,
            count: None,
            indent: 0.0,
            is_selected: false,
            role: TextRole::Body,
            forced: None,
        }
    }

    /// An icon before the text.
    pub fn glyph(mut self, icon: Icon) -> Self {
        self.glyph = Some(icon);
        self
    }

    /// A number at the right end. A test or a screen reader reads it as the value of the row.
    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Points of room before the glyph or the text.
    pub fn indent(mut self, points: f32) -> Self {
        self.indent = points;
        self
    }

    pub fn selected(mut self, is_selected: bool) -> Self {
        self.is_selected = is_selected;
        self
    }

    pub fn role(mut self, role: TextRole) -> Self {
        self.role = role;
        self
    }

    /// Draws the row as if the pointer or the keyboard were on it, unless `look` is `None`.
    pub(super) fn preview(mut self, look: Option<Look>) -> Self {
        self.forced = look;
        self
    }

    /// Paints the row, and says whether its text had to be cut.
    fn paint(&self, ui: &egui::Ui, rect: egui::Rect, look: Look) -> bool {
        let painter = ui.painter();
        let fill = if look.pressed {
            color::WELL_DEEP
        } else if self.is_selected || look.hovered {
            color::WELL
        } else {
            egui::Color32::TRANSPARENT
        };
        painter.rect_filled(rect, radius::SM, fill);
        if self.is_selected {
            let mut bar = rect;
            bar.max.x = bar.min.x + 2.0 * stroke::EDGE;
            painter.rect_filled(bar, radius::SM, color::RULE);
        }
        if look.focused {
            focus_ring(painter, rect, radius::SM);
        }

        let mut left = rect.left() + space::SM + self.indent;
        if let Some(glyph) = self.glyph {
            let centre = pos2(left + size::GLYPH / 2.0, rect.center().y);
            glyph.paint(painter, centre, size::GLYPH, color::TEXT_SECONDARY);
            left += size::GLYPH + space::SM;
        }
        let mut right = rect.right() - space::SM;
        if let Some(count) = self.count {
            let small = TextRole::Small;
            let galley = small.galley(ui, &count.to_string(), color::TEXT_MUTED);
            right -= galley.size().x;
            let top = rect.center().y - galley.size().y / 2.0;
            painter.galley(pos2(right, top), galley, color::TEXT_MUTED);
            right -= space::SM;
        }
        let mut job =
            LayoutJob::single_section(self.label.to_owned(), self.role.format(self.role.color()));
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = Some('…');
        job.wrap.max_width = (right - left).max(0.0);
        job.break_on_newline = false;
        let galley = painter.layout_job(job);
        let is_elided = galley.elided;
        let top = rect.center().y - galley.size().y / 2.0;
        painter.galley(pos2(left, top), galley, self.role.color());
        is_elided
    }
}

impl egui::Widget for ListRow<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let side = vec2(ui.available_width(), size::CONTROL_SM);
        let (rect, response) = ui.allocate_exact_size(side, Sense::click());
        response.widget_info(|| {
            WidgetInfo::selected(
                WidgetType::Button,
                ui.is_enabled(),
                self.is_selected,
                self.label,
            )
        });
        if let Some(count) = self.count {
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_value(count.to_string());
            });
        }
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let look = self.forced.unwrap_or_else(|| Look::of(&response));
        if self.paint(ui, rect, look) {
            response.on_hover_text(self.label)
        } else {
            response
        }
    }
}
