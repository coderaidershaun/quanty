//! The button in its five looks.

use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, Galley, Rect, Response, Sense, Stroke, StrokeKind, WidgetInfo,
    WidgetType, vec2,
};

use super::ControlSize;
use super::look::{Look, focus_ring};
use super::progress::paint_spinner;
use crate::theme::{Icon, TextRole, Tone, color, glow, radius, space, stroke};

const MIN_WIDTH_PER_HEIGHT: f32 = 2.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    Primary,
    Secondary,
    Ghost,
    Danger,
    IconOnly,
}

/// `primary` is the one main action of a screen, `secondary` is any other, `ghost` sits in a
/// toolbar, `danger` removes something, and `icon_only` is a small square with an icon.
pub struct Button<'a> {
    label: &'a str,
    variant: Variant,
    icon: Option<Icon>,
    size: ControlSize,
    height: Option<f32>,
    is_selected: bool,
    is_loading: bool,
    min_width: f32,
    forced: Option<Look>,
}

struct Colours {
    fill: Color32,
    edge: Stroke,
    text: Color32,
}

impl<'a> Button<'a> {
    fn with_variant(label: &'a str, variant: Variant) -> Self {
        Button {
            label,
            variant,
            icon: None,
            size: if variant == Variant::IconOnly {
                ControlSize::Small
            } else {
                ControlSize::Medium
            },
            height: None,
            is_selected: false,
            is_loading: false,
            min_width: 0.0,
            forced: None,
        }
    }

    pub fn primary(label: &'a str) -> Self {
        Button::with_variant(label, Variant::Primary)
    }

    pub fn secondary(label: &'a str) -> Self {
        Button::with_variant(label, Variant::Secondary)
    }

    pub fn ghost(label: &'a str) -> Self {
        Button::with_variant(label, Variant::Ghost)
    }

    pub fn danger(label: &'a str) -> Self {
        Button::with_variant(label, Variant::Danger)
    }

    /// `label` is the accessible name and the tooltip.
    pub fn icon_only(icon: Icon, label: &'a str) -> Self {
        Button {
            icon: Some(icon),
            ..Button::with_variant(label, Variant::IconOnly)
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// A primary or a danger button ignores it.
    pub fn selected(mut self, is_selected: bool) -> Self {
        self.is_selected = is_selected;
        self
    }

    pub fn loading(mut self, is_loading: bool) -> Self {
        self.is_loading = is_loading;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// The height in points, in place of the one its size gives. It is for a button that must
    /// fit inside another control.
    pub(super) fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Draws the button as if the pointer or the keyboard were on it.
    pub(super) fn preview(mut self, look: Look) -> Self {
        self.forced = Some(look);
        self
    }

    /// Whether the button keeps room before its text, for the icon or for the spinner that
    /// takes its place.
    fn has_icon_room(&self) -> bool {
        self.icon.is_some() || self.is_loading
    }

    fn colours(&self, look: Look) -> Colours {
        let solid = |tone: Tone| {
            let swatch = tone.swatch();
            let fill = if look.pressed {
                swatch.solid.lerp_to_gamma(color::CANVAS, 0.15)
            } else if look.hovered {
                swatch.solid.lerp_to_gamma(color::PAGE, 0.12)
            } else {
                swatch.solid
            };
            Colours {
                fill,
                edge: Stroke::NONE,
                text: swatch.on_solid,
            }
        };
        let quiet_fill = |idle: Color32| {
            if look.pressed {
                color::PANEL
            } else if look.hovered {
                color::RAISED_HOVER
            } else {
                idle
            }
        };
        match self.variant {
            Variant::Primary => solid(Tone::Magenta),
            Variant::Danger => solid(Tone::Danger),
            Variant::Secondary | Variant::Ghost | Variant::IconOnly if self.is_selected => {
                let blue = Tone::Blue.swatch();
                Colours {
                    fill: blue.wash,
                    edge: Stroke::new(stroke::BORDER, blue.edge),
                    text: blue.text,
                }
            }
            Variant::Secondary => Colours {
                fill: quiet_fill(color::RAISED),
                edge: Stroke::new(stroke::BORDER, color::BORDER),
                text: color::TEXT,
            },
            Variant::Ghost | Variant::IconOnly => Colours {
                fill: quiet_fill(Color32::TRANSPARENT),
                edge: Stroke::NONE,
                text: if look.hovered || look.pressed {
                    color::TEXT
                } else {
                    color::TEXT_SECONDARY
                },
            },
        }
    }

    fn paint(&self, ui: &egui::Ui, rect: Rect, look: Look, text: Option<Arc<Galley>>) {
        let colours = self.colours(look);
        let painter = ui.painter();
        if self.variant == Variant::Primary && ui.is_enabled() {
            painter.add(glow(Tone::Magenta).as_shape(rect, radius::MD));
        }
        painter.rect(
            rect,
            radius::MD,
            colours.fill,
            colours.edge,
            StrokeKind::Inside,
        );
        if look.focused {
            focus_ring(painter, rect, radius::MD);
        }

        let icon_size = self.size.icon();
        let has_icon = self.has_icon_room();
        let text_width = text.as_ref().map_or(0.0, |galley| galley.size().x);
        let gap = if has_icon && text.is_some() {
            space::SM
        } else {
            0.0
        };
        let icon_width = if has_icon { icon_size } else { 0.0 };
        let mut x = rect.center().x - (icon_width + gap + text_width) / 2.0;
        if has_icon {
            let centre = egui::pos2(x + icon_size / 2.0, rect.center().y);
            if self.is_loading {
                let spot = Rect::from_center_size(centre, egui::Vec2::splat(icon_size));
                paint_spinner(ui, spot, colours.text);
            } else if let Some(icon) = self.icon {
                painter.text(
                    centre,
                    Align2::CENTER_CENTER,
                    icon.glyph(),
                    Icon::font(icon_size),
                    colours.text,
                );
            }
            x += icon_width + gap;
        }
        if let Some(galley) = text {
            let top = rect.center().y - galley.size().y / 2.0;
            painter.galley_with_override_text_color(egui::pos2(x, top), galley, colours.text);
        }
    }
}

impl egui::Widget for Button<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let height = self.height.unwrap_or_else(|| self.size.height());
        let text = (self.variant != Variant::IconOnly)
            .then(|| TextRole::Label.galley(ui, self.label, color::TEXT));
        let width = match &text {
            None => height,
            Some(galley) => {
                // SMELL: a button with no icon gets wider while it loads, to make room for
                // the spinner, so what stands beside it moves.
                let icon_room = if self.has_icon_room() {
                    self.size.icon() + space::SM
                } else {
                    0.0
                };
                (galley.size().x + icon_room + 2.0 * space::LG)
                    .max(MIN_WIDTH_PER_HEIGHT * height)
                    .max(self.min_width)
            }
        };
        let sense = if self.is_loading {
            Sense::hover()
        } else {
            Sense::click()
        };
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);
        let is_enabled = ui.is_enabled() && !self.is_loading;
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, is_enabled, self.label));
        if ui.is_rect_visible(rect) {
            let look = if self.is_loading {
                Look::default()
            } else {
                self.forced.unwrap_or_else(|| Look::of(&response))
            };
            self.paint(ui, rect, look, text);
        }
        if self.variant == Variant::IconOnly {
            response.on_hover_text(self.label)
        } else {
            response
        }
    }
}
