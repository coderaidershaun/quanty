//! The button in its five looks.

use eframe::egui::{self, WidgetInfo, WidgetType};

use super::ControlSize;
use crate::theme::{Icon, TextRole, Tone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    Primary,
    Secondary,
    Ghost,
    Danger,
    IconOnly,
}

pub struct Button<'a> {
    label: &'a str,
    look: Look,
    size: ControlSize,
    selected: bool,
    loading: bool,
    min_width: f32,
}

impl<'a> Button<'a> {
    fn with_look(label: &'a str, look: Look) -> Self {
        Button {
            label,
            look,
            size: if look == Look::IconOnly {
                ControlSize::Small
            } else {
                ControlSize::Medium
            },
            selected: false,
            loading: false,
            min_width: 0.0,
        }
    }

    pub fn primary(label: &'a str) -> Self {
        Button::with_look(label, Look::Primary)
    }

    pub fn secondary(label: &'a str) -> Self {
        Button::with_look(label, Look::Secondary)
    }

    pub fn ghost(label: &'a str) -> Self {
        Button::with_look(label, Look::Ghost)
    }

    pub fn danger(label: &'a str) -> Self {
        Button::with_look(label, Look::Danger)
    }

    /// `label` is the accessible name and the tooltip.
    pub fn icon_only(_icon: Icon, label: &'a str) -> Self {
        Button::with_look(label, Look::IconOnly)
    }

    pub fn icon(self, _icon: Icon) -> Self {
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn selected(mut self, is_selected: bool) -> Self {
        self.selected = is_selected;
        self
    }

    pub fn loading(mut self, is_loading: bool) -> Self {
        self.loading = is_loading;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }
}

impl egui::Widget for Button<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut text = TextRole::Label.rich(self.label);
        match self.look {
            Look::Primary | Look::Danger => {
                let tone = if self.look == Look::Primary {
                    Tone::Magenta
                } else {
                    Tone::Danger
                };
                text = text.color(tone.swatch().on_solid);
            }
            Look::Secondary | Look::Ghost | Look::IconOnly => {}
        }
        let mut button = egui::Button::new(text)
            .min_size(egui::vec2(self.min_width, self.size.height()))
            .selected(self.selected);
        match self.look {
            Look::Primary => button = button.fill(Tone::Magenta.swatch().solid),
            Look::Danger => button = button.fill(Tone::Danger.swatch().solid),
            Look::Ghost | Look::IconOnly => button = button.frame(false),
            Look::Secondary => {}
        }
        let response = ui.add_enabled(!self.loading, button);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, !self.loading, self.label));
        if self.look == Look::IconOnly {
            response.on_hover_text(self.label)
        } else {
            response
        }
    }
}
