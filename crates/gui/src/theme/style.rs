//! The look of stock egui widgets: text styles, spacing, and the colour of every widget state, so
//! a check box, a menu or a scroll bar already matches the kit.

use eframe::egui::{self, Color32, CursorIcon, Margin, Shadow, Stroke, TextStyle, Vec2, style};

use super::{TextRole, Tone, color, motion, radius, size, space, stroke};

const SLIDER_WIDTH: f32 = 140.0;
const SLIDER_RAIL_HEIGHT: f32 = 4.0;
const TOOLTIP_WIDTH: f32 = 320.0;
const SCROLL_BAR_WIDTH: f32 = 8.0;
const TOOLTIP_DELAY_SECONDS: f32 = 0.4;
const DISABLED_ALPHA: f32 = 0.45;
const CARET_WIDTH: f32 = 2.0;

/// Sets the whole look. Both egui themes are given it, so a light system theme changes nothing.
pub(super) fn apply(style: &mut egui::Style) {
    text_styles(style);
    spacing(style);
    style.interaction.selectable_labels = false;
    style.interaction.tooltip_delay = TOOLTIP_DELAY_SECONDS;
    style.animation_time = motion::FAST;
    visuals(&mut style.visuals);
}

fn text_styles(style: &mut egui::Style) {
    style.text_styles = [
        (TextStyle::Small, TextRole::Small.font()),
        (TextStyle::Body, TextRole::Body.font()),
        (TextStyle::Button, TextRole::Label.font()),
        (TextStyle::Heading, TextRole::Heading.font()),
        (TextStyle::Monospace, TextRole::Mono.font()),
    ]
    .into();
}

fn spacing(style: &mut egui::Style) {
    let spacing = &mut style.spacing;
    spacing.item_spacing = Vec2::splat(space::SM);
    spacing.button_padding = egui::vec2(space::MD, space::XS);
    spacing.interact_size = Vec2::splat(size::CONTROL_SM);
    spacing.window_margin = Margin::from(space::LG);
    spacing.menu_margin = Margin::from(space::XS);
    spacing.icon_width = size::ICON_MD;
    spacing.slider_width = SLIDER_WIDTH;
    spacing.slider_rail_height = SLIDER_RAIL_HEIGHT;
    spacing.tooltip_width = TOOLTIP_WIDTH;
    spacing.scroll = style::ScrollStyle {
        bar_width: SCROLL_BAR_WIDTH,
        ..style::ScrollStyle::floating()
    };
}

fn visuals(visuals: &mut style::Visuals) {
    visuals.dark_mode = true;
    visuals.override_text_color = None;
    visuals.weak_text_color = Some(color::TEXT_MUTED);
    visuals.hyperlink_color = Tone::Blue.swatch().text;
    visuals.warn_fg_color = Tone::Warning.swatch().text;
    visuals.error_fg_color = Tone::Danger.swatch().text;

    visuals.panel_fill = color::CANVAS;
    visuals.window_fill = color::RAISED;
    visuals.extreme_bg_color = color::RAISED;
    visuals.faint_bg_color = color::RAISED;
    visuals.text_edit_bg_color = Some(color::RAISED);
    visuals.code_bg_color = color::RAISED_HOVER;
    visuals.window_stroke = Stroke::new(stroke::BORDER, color::BORDER);
    visuals.window_corner_radius = radius::LG.into();
    visuals.menu_corner_radius = radius::LG.into();
    let shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::BLACK.gamma_multiply(0.38),
    };
    visuals.window_shadow = shadow;
    visuals.popup_shadow = shadow;

    widget_states(&mut visuals.widgets);

    visuals.selection.bg_fill = color::SELECTION;
    visuals.selection.stroke = Stroke::new(stroke::BORDER, color::FOCUS);
    visuals.text_cursor.stroke = Stroke::new(CARET_WIDTH, color::TEXT);
    // A blinking caret would repaint an idle window for as long as a box has the focus.
    visuals.text_cursor.blink = false;
    visuals.handle_shape = style::HandleShape::Circle;
    visuals.slider_trailing_fill = true;
    visuals.interact_cursor = Some(CursorIcon::PointingHand);
    visuals.image_loading_spinners = false;
    visuals.disabled_alpha = DISABLED_ALPHA;
    visuals.striped = false;
}

fn widget_states(widgets: &mut style::Widgets) {
    let states = [
        (&mut widgets.noninteractive, color::PANEL, color::HAIRLINE),
        (&mut widgets.inactive, color::RAISED, color::BORDER),
        (&mut widgets.hovered, color::RAISED_HOVER, color::BORDER),
        (&mut widgets.active, color::RAISED_HOVER, color::FOCUS),
        (&mut widgets.open, color::RAISED_HOVER, color::FOCUS),
    ];
    for (state, fill, edge) in states {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(stroke::BORDER, edge);
        state.fg_stroke = Stroke::new(stroke::BORDER, color::TEXT);
        state.corner_radius = radius::MD.into();
        state.expansion = 0.0;
    }
}
