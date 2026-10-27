//! The bar under the page picture: zoom out, a slider, the percentage, zoom in, and fit to
//! width. A narrow panel has no room for the slider.

use eframe::egui;

use super::view::{MAX_ZOOM, MIN_ZOOM, View, Viewport, ZoomStep, step_zoom};
use crate::panels::PanelWidth;
use crate::theme::{Icon, TextRole};
use crate::widgets::{Button, Slider};

const SLIDER_WIDTH: f32 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ZoomAction {
    Step(ZoomStep),
    To(f32),
    Fit,
}

impl ZoomAction {
    pub(super) fn apply(self, viewport: &Viewport, view: View) -> View {
        let zoom = match self {
            ZoomAction::Step(step) => step_zoom(view.zoom, step),
            ZoomAction::To(zoom) => zoom,
            ZoomAction::Fit => 1.0,
        };
        viewport.zoom_about(view, viewport.room().center(), zoom)
    }
}

#[derive(Debug, Default)]
pub(super) struct Percent {
    rounded: u32,
    text: String,
}

impl Percent {
    fn text(&mut self, zoom: f32) -> &str {
        let rounded = percent_of(zoom);
        if rounded != self.rounded {
            self.rounded = rounded;
            self.text = format!("{rounded}%");
        }
        &self.text
    }
}

fn percent_of(zoom: f32) -> u32 {
    (zoom * 100.0).round() as u32
}

pub(super) fn show(
    ui: &mut egui::Ui,
    zoom: f32,
    percent: &mut Percent,
    width: PanelWidth,
) -> Option<ZoomAction> {
    let size = width.control_size();
    let has_slider = width == PanelWidth::Roomy;
    let mut action = None;
    ui.spacing_mut().slider_width = SLIDER_WIDTH;
    if has_slider {
        ui.label(TextRole::Small.rich("Zoom"));
    }
    let can_zoom_out = step_zoom(zoom, ZoomStep::Out) < zoom;
    let zoom_out = Button::icon_only(Icon::MINUS, "Zoom out").size(size);
    if ui.add_enabled(can_zoom_out, zoom_out).clicked() {
        action = Some(ZoomAction::Step(ZoomStep::Out));
    }
    let range = MIN_ZOOM.log2()..=MAX_ZOOM.log2();
    if has_slider && let Some(value) = Slider::new("Zoom", zoom.log2(), range).show(ui) {
        action = Some(ZoomAction::To(value.exp2()));
    }
    ui.label(TextRole::Small.rich(percent.text(zoom)));
    let can_zoom_in = step_zoom(zoom, ZoomStep::In) > zoom;
    let zoom_in = Button::icon_only(Icon::PLUS, "Zoom in").size(size);
    if ui.add_enabled(can_zoom_in, zoom_in).clicked() {
        action = Some(ZoomAction::Step(ZoomStep::In));
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let fit = Button::icon_only(Icon::FIT, "Fit to width").size(size);
        if ui.add_enabled(percent_of(zoom) != 100, fit).clicked() {
            action = Some(ZoomAction::Fit);
        }
    });
    action
}
