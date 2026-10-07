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

/// A text button is at least this many times as wide as it is high.
const MIN_WIDTH_PER_HEIGHT: f32 = 2.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    Primary,
    Secondary,
    Ghost,
    Danger,
    IconOnly,
}

/// A button. `primary` is the one main action of a screen, `secondary` is any other, `ghost`
/// sits in a toolbar, `danger` removes something, and `icon_only` is a small square with an icon.
pub struct Button<'a> {
    label: &'a str,
    variant: Variant,
    icon: Option<Icon>,
    size: ControlSize,
    selected: bool,
    loading: bool,
    min_width: f32,
    forced: Option<Look>,
}

/// The three colours a button is painted with in one state.
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
            selected: false,
            loading: false,
            min_width: 0.0,
            forced: None,
        }
    }

    /// The one main action of a screen, in magenta.
    pub fn primary(label: &'a str) -> Self {
        Button::with_variant(label, Variant::Primary)
    }

    /// An action with an outline.
    pub fn secondary(label: &'a str) -> Self {
        Button::with_variant(label, Variant::Secondary)
    }

    /// An action with no fill until the pointer is on it.
    pub fn ghost(label: &'a str) -> Self {
        Button::with_variant(label, Variant::Ghost)
    }

    /// An action that removes something.
    pub fn danger(label: &'a str) -> Self {
        Button::with_variant(label, Variant::Danger)
    }

    /// `label` is the accessible name and the tooltip.
    pub fn icon_only(_icon: Icon, label: &'a str) -> Self {
        Button {
            icon: Some(_icon),
            ..Button::with_variant(label, Variant::IconOnly)
        }
    }

    /// An icon before the text.
    pub fn icon(self, _icon: Icon) -> Self {
        Button {
            icon: Some(_icon),
            ..self
        }
    }

    /// The height of the button. A text button is `Medium` by default and an icon-only one `Small`.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Drawn as chosen. The two solid looks ignore it.
    pub fn selected(mut self, is_selected: bool) -> Self {
        self.selected = is_selected;
        self
    }

    /// A spinner takes the place of the icon, and clicks are ignored.
    pub fn loading(mut self, is_loading: bool) -> Self {
        self.loading = is_loading;
        self
    }

    /// The button is at least this wide.
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// Draws the button as if the pointer or the keyboard were on it.
    pub(super) fn preview(mut self, look: Look) -> Self {
        self.forced = Some(look);
        self
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
            Variant::Secondary | Variant::Ghost | Variant::IconOnly if self.selected => {
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
        let has_icon = self.icon.is_some() || self.loading;
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
            if self.loading {
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
        let height = self.size.height();
        let text = (self.variant != Variant::IconOnly)
            .then(|| TextRole::Label.galley(ui, self.label, color::TEXT));
        let width = match &text {
            None => height,
            Some(galley) => {
                let icon = if self.icon.is_some() || self.loading {
                    self.size.icon() + space::SM
                } else {
                    0.0
                };
                (galley.size().x + icon + 2.0 * space::LG)
                    .max(MIN_WIDTH_PER_HEIGHT * height)
                    .max(self.min_width)
            }
        };
        let sense = if self.loading {
            Sense::hover()
        } else {
            Sense::click()
        };
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);
        let is_enabled = ui.is_enabled() && !self.loading;
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, is_enabled, self.label));
        if ui.is_rect_visible(rect) {
            let look = if self.loading {
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
