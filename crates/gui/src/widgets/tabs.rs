//! The strip of tabs along the top of a window or of a panel.

use eframe::egui::{
    self, Align2, Color32, WidgetInfo, WidgetType, accesskit::Role, text::LayoutJob,
};

use super::look::{Look, focus_ring};
use crate::theme::{Icon, TextRole, Tone, color, hairline, radius, size, space, stroke};

/// One tab of a strip. It shows its label, then its count in brackets when it has one. The
/// label alone is the accessible name, and a test or a screen reader reads the count as the
/// value of the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tab<'a> {
    pub label: &'a str,
    pub count: Option<usize>,
    pub icon: Option<Icon>,
}

impl<'a> Tab<'a> {
    pub const fn new(label: &'a str) -> Self {
        Tab {
            label,
            count: None,
            icon: None,
        }
    }

    pub const fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    pub const fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// A row of tabs. It reports the tab that was chosen and keeps no choice of its own.
pub struct TabStrip<'a> {
    tabs: &'a [Tab<'a>],
    active: usize,
    tone: Tone,
    is_compact: bool,
    forced: Option<Look>,
}

impl<'a> TabStrip<'a> {
    pub fn new(tabs: &'a [Tab<'a>], active: usize) -> Self {
        TabStrip {
            tabs,
            active,
            tone: Tone::Blue,
            is_compact: false,
            forced: None,
        }
    }

    /// The colour of the active tab and of the bar under it. It is blue when this is not called.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// A smaller strip for a narrow column.
    pub fn compact(mut self) -> Self {
        self.is_compact = true;
        self
    }

    /// Draws every tab as if the pointer or the keyboard were on it.
    pub(super) fn preview(mut self, look: Look) -> Self {
        self.forced = Some(look);
        self
    }

    /// The index of the tab the person chose. Never the active one.
    pub fn show(self, ui: &mut egui::Ui) -> Option<usize> {
        let (role, gap, height) = if self.is_compact {
            (TextRole::Small, space::LG, size::CONTROL_MD)
        } else {
            (TextRole::Label, space::XL, size::TAB)
        };
        let start = ui.cursor().min.x;
        let full_width = ui.available_width();
        let mut chosen = None;
        let row = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (index, tab) in self.tabs.iter().enumerate() {
                if self.show_tab(ui, tab, index == self.active, role, height) {
                    chosen = Some(index);
                }
            }
        });
        // In a room with no end, such as a sideways scroll area, the line stops at the last tab.
        let end = if full_width.is_finite() {
            start + full_width
        } else {
            row.response.rect.right()
        };
        let painter = ui.painter();
        let y = row.response.rect.bottom();
        painter.hline(start..=end, y, hairline(painter, color::HAIRLINE));
        chosen
    }

    /// Draws one tab and says whether it was clicked and is not the active one.
    fn show_tab(
        &self,
        ui: &mut egui::Ui,
        tab: &Tab<'_>,
        is_active: bool,
        role: TextRole,
        height: f32,
    ) -> bool {
        let swatch = self.tone.swatch();
        let icon_size = size::ICON_MD;
        let icon_room = if tab.icon.is_some() {
            icon_size + space::SM
        } else {
            0.0
        };
        // The colour does not change the width, so any colour measures the text.
        let measured = self.galley(ui, tab, role, color::TEXT);
        let width = icon_room + measured.size().x;
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
        // egui has no widget type for a tab, so the tab is named as a button and is given its
        // role afterwards.
        response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), tab.label));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(Role::Tab);
            node.set_selected(is_active);
            if let Some(count) = tab.count {
                node.set_value(count.to_string());
            }
        });
        if ui.is_rect_visible(rect) {
            let look = self.forced.unwrap_or_else(|| Look::of(&response));
            let tint = if is_active {
                swatch.text
            } else if look.hovered || look.pressed {
                color::TEXT
            } else {
                color::TEXT_SECONDARY
            };
            let painter = ui.painter();
            let mut x = rect.left();
            if let Some(icon) = tab.icon {
                painter.text(
                    egui::pos2(x + icon_size / 2.0, rect.center().y),
                    Align2::CENTER_CENTER,
                    icon.glyph(),
                    Icon::font(icon_size),
                    tint,
                );
                x += icon_room;
            }
            let galley = self.galley(ui, tab, role, tint);
            let top = rect.center().y - galley.size().y / 2.0;
            painter.galley(egui::pos2(x, top), galley, tint);
            if is_active {
                let mut bar = rect;
                bar.min.y = rect.bottom() - stroke::UNDERLINE;
                painter.rect_filled(bar, stroke::UNDERLINE / 2.0, swatch.solid);
            }
            if look.focused {
                focus_ring(painter, rect, radius::SM);
            }
        }
        response.clicked() && !is_active
    }

    /// The label, and after it the count in brackets. A count of zero is muted.
    fn galley(
        &self,
        ui: &egui::Ui,
        tab: &Tab<'_>,
        role: TextRole,
        tint: Color32,
    ) -> std::sync::Arc<egui::Galley> {
        let mut job = LayoutJob::default();
        job.append(tab.label, 0.0, role.format(tint));
        if let Some(count) = tab.count {
            let tint = if count == 0 { color::TEXT_MUTED } else { tint };
            job.append(&format!(" ({count})"), 0.0, role.format(tint));
        }
        ui.painter().layout_job(job)
    }
}
