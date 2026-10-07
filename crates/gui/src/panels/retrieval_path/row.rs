//! One row of the path, laid out by hand in the rectangle it is given, and the line that joins
//! two rows.

use eframe::egui;

use super::steps::{Progress, Step};
use crate::theme::{self, TextRole, Tone, color, size, space};
use crate::widgets::{Badge, StepMarker, StepState};

const TITLE: f32 = TextRole::Label.line_height();
const TWO_LINES: f32 = TITLE + TextRole::Small.line_height();
const MAX_PITCH: f32 = TWO_LINES + space::MD;
const HALF_STEP: f32 = size::STEP / 2.0;

/// Whether a row has room for the line under its title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Density {
    Full,
    Compact,
}

/// The rows share the height they are given, up to the height of two lines and a gap.
pub(super) fn fit(height: f32, rows: usize) -> (f32, Density) {
    let pitch = (height / rows as f32).floor().clamp(size::STEP, MAX_PITCH);
    let density = if pitch >= TWO_LINES + space::XXS {
        Density::Full
    } else {
        Density::Compact
    };
    (pitch, density)
}

pub(super) fn show(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    number: usize,
    step: &Step,
    density: Density,
) {
    let marker = egui::Rect::from_center_size(
        egui::pos2(rect.left() + HALF_STEP, rect.center().y),
        egui::Vec2::splat(size::STEP),
    );
    let state = match step.progress {
        Progress::Taken => StepState::Done,
        Progress::Waiting | Progress::NotReached => StepState::Pending,
    };
    ui.place(marker, StepMarker::new(number).tone(step.tone).state(state));

    let block = match density {
        Density::Full => TWO_LINES,
        Density::Compact => TITLE,
    };
    let top = (rect.center().y - block / 2.0).round();
    let left = marker.right() + space::SM;
    let (title_color, line_color) = match step.progress {
        Progress::NotReached => (color::TEXT_MUTED, color::TEXT_MUTED),
        Progress::Waiting | Progress::Taken => (color::TEXT, color::TEXT_SECONDARY),
    };

    // The badge sits on the title's line, so the line under the title can use the whole width.
    let title_line = egui::Rect::from_min_max(
        egui::pos2(rect.left(), top),
        egui::pos2(rect.right(), top + TITLE),
    );
    let title_right = match &step.badge {
        Some(text) => badge_in(ui, title_line, text).rect.left() - space::SM,
        None => rect.right(),
    };
    let title_rect =
        egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(title_right, top + TITLE));
    let title = label_in(
        ui,
        title_rect,
        TextRole::Label.rich(step.title).color(title_color),
    );
    match density {
        Density::Full => {
            let line_rect = egui::Rect::from_min_max(
                egui::pos2(left, top + TITLE),
                egui::pos2(rect.right(), top + TWO_LINES),
            );
            label_in(
                ui,
                line_rect,
                TextRole::Small.rich(&step.line).color(line_color),
            );
        }
        Density::Compact => {
            title.on_hover_text(&step.line);
        }
    }
}

/// A short line between two markers, drawn only when the rows are far enough apart for it.
pub(super) fn connect(ui: &egui::Ui, upper: egui::Rect, lower: egui::Rect) {
    let x = upper.left() + HALF_STEP;
    let from = upper.center().y + HALF_STEP + space::XS;
    let to = lower.center().y - HALF_STEP - space::XS;
    if to - from >= space::XS {
        let painter = ui.painter();
        painter.vline(x, from..=to, theme::hairline(painter, color::BORDER));
    }
}

// A child made with new_child leaves the cursor alone. A scope would move it, and the rows
// would overlap.
fn label_in(ui: &mut egui::Ui, rect: egui::Rect, text: egui::RichText) -> egui::Response {
    let across = egui::Layout::left_to_right(egui::Align::Center);
    ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(across))
        .add(egui::Label::new(text).truncate())
}

fn badge_in(ui: &mut egui::Ui, rect: egui::Rect, text: &str) -> egui::Response {
    let across = egui::Layout::right_to_left(egui::Align::Center);
    ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(across))
        .add(Badge::new(text).tone(Tone::Neutral))
}
