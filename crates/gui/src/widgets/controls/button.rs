//! The button in its five looks: a block of ink, an outline, plain text, red text that is
//! outlined only while the pointer is on it, and a small square with an icon.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, Galley, Rect, Response, Sense, Stroke, StrokeKind, WidgetInfo, WidgetType, vec2,
};

use crate::theme::{Icon, TextRole, Tone, color, radius, space, stroke};
use crate::widgets::ControlSize;
use crate::widgets::look::{Look, focus_ring};
use crate::widgets::progress::paint_spinner;

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
    accessible_name: Option<&'a str>,
    variant: Variant,
    icon: Option<Icon>,
    size: ControlSize,
    height: Option<f32>,
    is_selected: bool,
    is_loading: bool,
    min_width: f32,
    is_start_aligned: bool,
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
            accessible_name: None,
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
            is_start_aligned: false,
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

    /// The least width of a primary, secondary or danger button with a label, which is the whole
    /// width of one whose label is short. A ghost button has no least width.
    pub const fn least_width(size: ControlSize) -> f32 {
        MIN_WIDTH_PER_HEIGHT * size.height()
    }

    /// `label` is the accessible name and the tooltip.
    pub fn icon_only(icon: Icon, label: &'a str) -> Self {
        Button {
            icon: Some(icon),
            ..Button::with_variant(label, Variant::IconOnly)
        }
    }

    /// The accessible name and the hover text, in place of the label, for a button whose label
    /// alone does not say what it acts on.
    pub fn accessible_name(mut self, name: &'a str) -> Self {
        self.accessible_name = Some(name);
        self
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

    /// The icon and the label start at the left padding and do not stand in the middle, for a
    /// row of a menu.
    pub fn start_aligned(mut self) -> Self {
        self.is_start_aligned = true;
        self
    }

    /// The height in points, in place of the one its size gives. It is for a button that must
    /// fit inside another control.
    pub(super) fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Draws the button as if the pointer or the keyboard were on it, unless `look` is `None`.
    pub(in crate::widgets) fn preview(mut self, look: Option<Look>) -> Self {
        self.forced = look;
        self
    }

    /// Whether the button keeps room before its text, for the icon or for the spinner that
    /// takes its place.
    fn has_icon_room(&self) -> bool {
        self.icon.is_some() || self.is_loading
    }

    /// The room left and right of the icon and the label.
    fn padding(&self) -> f32 {
        match self.variant {
            Variant::Ghost => space::SM,
            _ => space::LG,
        }
    }

    fn width_for_text(&self, text_width: f32, height: f32) -> f32 {
        // A primary, secondary or danger button with no icon draws its spinner in its wide
        // padding, so it keeps its width while it loads and nothing beside it moves. The padding
        // of a ghost button is too narrow for a spinner, so it makes room for one as for an icon.
        let (least_width, keeps_icon_room) = match self.variant {
            Variant::Ghost => (0.0, self.has_icon_room()),
            _ => (MIN_WIDTH_PER_HEIGHT * height, self.icon.is_some()),
        };
        let padding = self.padding();
        let icon_room = if keeps_icon_room {
            self.size.icon() + space::SM
        } else {
            0.0
        };
        (text_width + icon_room + 2.0 * padding)
            .max(least_width)
            .max(self.min_width)
    }

    fn colours(&self, look: Look) -> Colours {
        let quiet_fill = if look.pressed {
            color::WELL_DEEP
        } else if look.hovered {
            color::WELL
        } else {
            Color32::TRANSPARENT
        };
        match self.variant {
            Variant::Primary => Colours {
                fill: if look.pressed {
                    color::INK_PRESSED
                } else if look.hovered {
                    color::INK_HOVER
                } else {
                    color::INK
                },
                edge: Stroke::NONE,
                text: color::TEXT_ON_INK,
            },
            Variant::Danger => {
                let danger = Tone::Danger.swatch();
                let (fill, edge) = if look.pressed {
                    (danger.wash, Stroke::new(stroke::BORDER, danger.solid))
                } else if look.hovered {
                    (danger.wash, Stroke::new(stroke::BORDER, danger.edge))
                } else {
                    (Color32::TRANSPARENT, Stroke::NONE)
                };
                Colours {
                    fill,
                    edge,
                    text: danger.text,
                }
            }
            Variant::Secondary | Variant::Ghost | Variant::IconOnly if self.is_selected => {
                Colours {
                    fill: if look.hovered || look.pressed {
                        color::WELL_DEEP
                    } else {
                        color::WELL
                    },
                    edge: Stroke::new(stroke::EDGE, color::RULE),
                    text: color::TEXT,
                }
            }
            Variant::Secondary => Colours {
                fill: quiet_fill,
                edge: Stroke::new(stroke::BORDER, color::BORDER),
                text: color::TEXT,
            },
            Variant::Ghost | Variant::IconOnly => Colours {
                fill: quiet_fill,
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
        let mut x = if self.is_start_aligned {
            rect.left() + self.padding()
        } else {
            rect.center().x - (icon_width + gap + text_width) / 2.0
        };
        if has_icon {
            let centre = egui::pos2(x + icon_size / 2.0, rect.center().y);
            if self.is_loading {
                let spot = Rect::from_center_size(centre, egui::Vec2::splat(icon_size));
                paint_spinner(ui, spot, colours.text);
            } else if let Some(icon) = self.icon {
                icon.paint(painter, centre, icon_size, colours.text);
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
            Some(galley) => self.width_for_text(galley.size().x, height),
        };
        let sense = if self.is_loading {
            Sense::hover()
        } else {
            Sense::click()
        };
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);
        crate::widgets::interactive_cursor(ui, &response);
        let is_enabled = ui.is_enabled() && !self.is_loading;
        let name = self.accessible_name.unwrap_or(self.label);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, is_enabled, name));
        if ui.is_rect_visible(rect) {
            let look = if self.is_loading {
                Look::default()
            } else {
                self.forced.unwrap_or_else(|| Look::of(&response))
            };
            self.paint(ui, rect, look, text);
        }
        if self.variant == Variant::IconOnly || self.accessible_name.is_some() {
            response.on_hover_text(name)
        } else {
            response
        }
    }
}
