//! The switch between the dark and the light palette: an icon button in the top bar, or a row of
//! the footer's popup on a phone.

use eframe::egui;

use crate::panels::PanelCx;
use crate::theme::{self, Icon, Scheme};
use crate::widgets::{Button, ControlSize};
use contract::Intent;

/// The icon and the name of the switch while the window draws `scheme`. Both say what a click
/// turns the window to, so they always agree with the palette on screen.
fn words(scheme: Scheme) -> (Icon, &'static str) {
    match scheme {
        Scheme::Dark => (Icon::SUN, "Switch to light mode"),
        Scheme::Light => (Icon::MOON, "Switch to dark mode"),
    }
}

/// The small square of the top bar.
pub(super) fn button(ui: &mut egui::Ui, size: ControlSize, cx: &mut PanelCx<'_>) {
    let (icon, name) = words(theme::scheme(ui.ctx()));
    if ui.add(Button::icon_only(icon, name).size(size)).clicked() {
        cx.intents.push(Intent::SwitchScheme);
    }
}

/// The row of the footer's popup, which has the room to say it in words.
pub(super) fn row(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    let (icon, name) = words(theme::scheme(ui.ctx()));
    let button = Button::secondary(name).icon(icon).size(ControlSize::Medium);
    if ui.add(button).clicked() {
        cx.intents.push(Intent::SwitchScheme);
    }
}
