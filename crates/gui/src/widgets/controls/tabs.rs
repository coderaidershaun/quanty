//! The strip of tabs along the top of a window or of a panel. A strip that has less room than its
//! tabs need scrolls sideways and keeps the open tab in view.

use eframe::egui::{
    self, Color32, WidgetInfo, WidgetType,
    accesskit::Role,
    scroll_area::{ScrollBarVisibility, ScrollSource},
    style::ScrollAnimation,
    text::LayoutJob,
};

use crate::theme::{TextRole, color, hairline, radius, size, space, stroke};
use crate::widgets::look::{Look, focus_ring};

/// The label alone is the accessible name, and a test or a screen reader reads the count as the
/// value of the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tab<'a> {
    pub label: &'a str,
    pub count: Option<usize>,
}

impl<'a> Tab<'a> {
    pub const fn new(label: &'a str) -> Self {
        Tab { label, count: None }
    }

    pub const fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// egui has no widget type for a tab, so the tab is named as a button and is given its role
    /// afterwards.
    pub(super) fn announce(&self, ui: &egui::Ui, response: &egui::Response, is_active: bool) {
        response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), self.label));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(Role::Tab);
            node.set_selected(is_active);
            if let Some(count) = self.count {
                node.set_value(count.to_string());
            }
        });
    }
}

pub(super) fn tab_tint(is_active: bool, look: Look) -> Color32 {
    if is_active || look.hovered || look.pressed {
        color::TEXT
    } else {
        color::TEXT_SECONDARY
    }
}

/// How the tabs of one strip are drawn.
#[derive(Clone, Copy)]
struct Metrics {
    role: TextRole,
    gap: f32,
    height: f32,
}

pub struct TabStrip<'a> {
    tabs: &'a [Tab<'a>],
    active: usize,
    is_compact: bool,
    forced: Option<Look>,
}

impl<'a> TabStrip<'a> {
    pub fn new(tabs: &'a [Tab<'a>], active: usize) -> Self {
        TabStrip {
            tabs,
            active,
            is_compact: false,
            forced: None,
        }
    }

    pub fn compact(mut self) -> Self {
        self.is_compact = true;
        self
    }

    /// Draws every tab as if the pointer or the keyboard were on it.
    pub(in crate::widgets) fn preview(mut self, look: Look) -> Self {
        self.forced = Some(look);
        self
    }

    /// The index of the tab the person chose. Never the active one.
    pub fn show(self, ui: &mut egui::Ui) -> Option<usize> {
        let metrics = if self.is_compact {
            Metrics {
                role: TextRole::Small,
                gap: space::LG,
                height: size::CONTROL_MD,
            }
        } else {
            Metrics {
                role: TextRole::Label,
                gap: space::XL,
                height: size::TAB,
            }
        };
        let start = ui.cursor().min.x;
        let full_width = ui.available_width();
        let (chosen, bottom, end) =
            if full_width.is_finite() && self.width_needed(ui, metrics) > full_width {
                let (chosen, bottom) = self.show_scrolling(ui, metrics, full_width);
                (chosen, bottom, start + full_width)
            } else {
                let row = ui.horizontal(|ui| self.show_row(ui, metrics));
                let (chosen, _) = row.inner;
                // In a room with no end, such as a sideways scroll area, the line stops at the last
                // tab.
                let end = if full_width.is_finite() {
                    start + full_width
                } else {
                    row.response.rect.right()
                };
                (chosen, row.response.rect.bottom(), end)
            };
        let painter = ui.painter();
        painter.hline(start..=end, bottom, hairline(painter, color::HAIRLINE));
        chosen
    }

    /// How wide the tabs are side by side, with the gap between them.
    fn width_needed(&self, ui: &egui::Ui, metrics: Metrics) -> f32 {
        let widths: f32 = self
            .tabs
            .iter()
            .map(|tab| self.galley(ui, tab, metrics.role, color::TEXT).size().x)
            .sum();
        widths + metrics.gap * self.tabs.len().saturating_sub(1) as f32
    }

    /// The tabs in a row that slides sideways under the pointer, a finger or the wheel. The open
    /// tab is brought into view whenever it or the room changes. Returns the tab that was chosen,
    /// and the bottom of the row.
    fn show_scrolling(
        &self,
        ui: &mut egui::Ui,
        metrics: Metrics,
        room: f32,
    ) -> (Option<usize>, f32) {
        let labels: Vec<&str> = self.tabs.iter().map(|tab| tab.label).collect();
        let seen_id = ui.make_persistent_id(("tab strip seen", &labels));
        let has_changed =
            ui.data(|data| data.get_temp::<(usize, f32)>(seen_id)) != Some((self.active, room));
        ui.data_mut(|data| data.insert_temp(seen_id, (self.active, room)));
        ui.scope(|ui| {
            // Without this the wheel scrolls a sideways area only while shift is held.
            ui.style_mut().always_scroll_the_only_direction = true;
            egui::ScrollArea::horizontal()
                .id_salt(("tab strip", &labels))
                .scroll_source(ScrollSource::ALL)
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    let row = ui.horizontal(|ui| self.show_row(ui, metrics));
                    let (chosen, active) = row.inner;
                    if has_changed && let Some(active) = active {
                        active.scroll_to_me_animation(None, ScrollAnimation::none());
                    }
                    (chosen, row.response.rect.bottom())
                })
                .inner
        })
        .inner
    }

    /// Draws the tabs one after the other. Returns the tab that was chosen, and the response of the
    /// open tab.
    fn show_row(
        &self,
        ui: &mut egui::Ui,
        metrics: Metrics,
    ) -> (Option<usize>, Option<egui::Response>) {
        ui.spacing_mut().item_spacing.x = metrics.gap;
        let mut chosen = None;
        let mut active = None;
        for (index, tab) in self.tabs.iter().enumerate() {
            let is_active = index == self.active;
            let response = self.show_tab(ui, tab, is_active, metrics);
            if response.clicked() && !is_active {
                chosen = Some(index);
            }
            if is_active {
                active = Some(response);
            }
        }
        (chosen, active)
    }

    fn show_tab(
        &self,
        ui: &mut egui::Ui,
        tab: &Tab<'_>,
        is_active: bool,
        metrics: Metrics,
    ) -> egui::Response {
        let Metrics { role, height, .. } = metrics;
        // The colour does not change the width, so any colour measures the text.
        let measured = self.galley(ui, tab, role, color::TEXT);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(measured.size().x, height), egui::Sense::click());
        crate::widgets::interactive_cursor(ui, &response);
        tab.announce(ui, &response, is_active);
        if ui.is_rect_visible(rect) {
            let look = self.forced.unwrap_or_else(|| Look::of(&response));
            let tint = tab_tint(is_active, look);
            let painter = ui.painter();
            let galley = self.galley(ui, tab, role, tint);
            let top = rect.center().y - galley.size().y / 2.0;
            painter.galley(egui::pos2(rect.left(), top), galley, tint);
            if is_active {
                let mut bar = rect;
                bar.min.y = rect.bottom() - stroke::UNDERLINE;
                painter.rect_filled(bar, 0.0, color::RULE);
            }
            if look.focused {
                focus_ring(painter, rect, radius::SM);
            }
        }
        response
    }

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
