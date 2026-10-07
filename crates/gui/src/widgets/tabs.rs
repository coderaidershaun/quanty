//! The strip of tabs along the top of a window.

use eframe::egui::{self, accesskit::Role};

use crate::theme::{Icon, TextRole, Tone, size, space};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub struct TabStrip<'a> {
    tabs: &'a [Tab<'a>],
    active: usize,
    tone: Tone,
    compact: bool,
}

impl<'a> TabStrip<'a> {
    pub fn new(tabs: &'a [Tab<'a>], active: usize) -> Self {
        TabStrip {
            tabs,
            active,
            tone: Tone::Blue,
            compact: false,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// A smaller strip for a narrow column.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// The index of the tab the person chose. Never the active one.
    pub fn show(self, ui: &mut egui::Ui) -> Option<usize> {
        let mut chosen = None;
        let (role, gap, height) = if self.compact {
            (TextRole::Small, space::LG, size::CONTROL_MD)
        } else {
            (TextRole::Label, space::XL, size::TAB)
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.set_min_height(height);
            for (index, tab) in self.tabs.iter().enumerate() {
                let is_active = index == self.active;
                let color = if is_active {
                    self.tone.swatch().text
                } else {
                    crate::theme::color::TEXT_SECONDARY
                };
                let text = match tab.count {
                    Some(count) => format!("{} ({count})", tab.label),
                    None => tab.label.to_owned(),
                };
                let response = ui.selectable_label(is_active, role.rich(text).color(color));
                ui.ctx().accesskit_node_builder(response.id, |node| {
                    node.set_role(Role::Tab);
                    node.set_label(tab.label);
                });
                if response.clicked() && !is_active {
                    chosen = Some(index);
                }
            }
        });
        chosen
    }
}
