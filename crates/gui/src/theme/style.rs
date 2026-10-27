//! The look of stock egui widgets: text styles, spacing, and the colour of every widget state, so
//! a check box, a menu or a scroll bar already matches the kit.

use eframe::egui::{self, CursorIcon, Margin, Stroke, TextStyle, Vec2, style};

use super::{Palette, Scheme, TextRole, Tone, motion, radius, shadow, size, space, stroke};

const SLIDER_WIDTH: f32 = 140.0;
const SLIDER_RAIL_HEIGHT: f32 = 4.0;
const TOOLTIP_WIDTH: f32 = 320.0;
const SCROLL_BAR_WIDTH: f32 = 8.0;
const TOOLTIP_DELAY_SECONDS: f32 = 0.4;
const DISABLED_ALPHA: f32 = 0.45;
const CARET_WIDTH: f32 = 2.0;

/// Both egui themes are given this look, each in the colours of its own scheme, so the theme of
/// the system changes nothing.
pub(super) fn apply(style: &mut egui::Style, scheme: Scheme) {
    text_styles(style);
    spacing(style);
    style.interaction.selectable_labels = false;
    style.interaction.tooltip_delay = TOOLTIP_DELAY_SECONDS;
    style.animation_time = motion::FAST;
    visuals(&mut style.visuals, scheme);
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

/// Each scheme starts from the egui visuals of its own kind, because they carry the text colour
/// curve that suits it. Dark text on a light fill and light text on a dark one need different
/// curves to look equally strong.
fn visuals(visuals: &mut style::Visuals, scheme: Scheme) {
    let palette = scheme.palette();
    *visuals = match scheme {
        Scheme::Light => style::Visuals::light(),
        Scheme::Dark => style::Visuals::dark(),
    };
    visuals.override_text_color = None;
    visuals.weak_text_color = Some(palette.text_muted);
    visuals.hyperlink_color = palette.tone(Tone::Indigo).text;
    visuals.warn_fg_color = palette.tone(Tone::Warning).text;
    visuals.error_fg_color = palette.tone(Tone::Danger).text;

    visuals.panel_fill = palette.desk;
    visuals.window_fill = palette.sheet;
    visuals.extreme_bg_color = palette.well;
    visuals.faint_bg_color = palette.well;
    visuals.text_edit_bg_color = Some(palette.well);
    visuals.code_bg_color = palette.well;
    visuals.window_stroke = Stroke::new(stroke::BORDER, palette.border);
    visuals.window_corner_radius = radius::MD.into();
    visuals.menu_corner_radius = radius::MD.into();
    visuals.window_shadow = shadow::sheet(palette);
    visuals.popup_shadow = shadow::sheet(palette);

    widget_states(&mut visuals.widgets, palette);

    visuals.selection.bg_fill = palette.selection;
    visuals.selection.stroke = Stroke::new(stroke::BORDER, palette.focus);
    visuals.text_cursor.stroke = Stroke::new(CARET_WIDTH, palette.text);
    // A blinking caret would repaint an idle window for as long as a box has the focus.
    visuals.text_cursor.blink = false;
    visuals.handle_shape = style::HandleShape::Circle;
    visuals.slider_trailing_fill = true;
    visuals.interact_cursor = Some(CursorIcon::PointingHand);
    visuals.image_loading_spinners = false;
    visuals.disabled_alpha = DISABLED_ALPHA;
    visuals.striped = false;
}

fn widget_states(widgets: &mut style::Widgets, palette: &Palette) {
    let states = [
        (&mut widgets.noninteractive, palette.sheet, palette.hairline),
        (&mut widgets.inactive, palette.sheet, palette.border),
        (&mut widgets.hovered, palette.well, palette.border),
        (&mut widgets.active, palette.well_deep, palette.rule),
        (&mut widgets.open, palette.well, palette.rule),
    ];
    for (state, fill, edge) in states {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(stroke::BORDER, edge);
        state.fg_stroke = Stroke::new(stroke::BORDER, palette.text);
        state.corner_radius = radius::MD.into();
        state.expansion = 0.0;
    }
}
