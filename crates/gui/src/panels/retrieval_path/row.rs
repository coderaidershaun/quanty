//! One row of the path, laid out by hand in the rectangle it is given, and the line that joins
//! two rows.

use eframe::egui;

use super::steps::{Progress, Step};
use crate::theme::{self, Palette, TextRole, size, space};
use crate::widgets::{StepMarker, StepState};

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
    let marker = place_marker(ui, rect, number, step);
    let block = match density {
        Density::Full => TWO_LINES,
        Density::Compact => TITLE,
    };
    let top = (rect.center().y - block / 2.0).round();
    let left = marker.right() + space::SM;
    let title = title_line(ui, rect, egui::pos2(left, top), step);
    match density {
        Density::Full => {
            let line_rect = egui::Rect::from_min_max(
                egui::pos2(left, top + TITLE),
                egui::pos2(rect.right(), top + TWO_LINES),
            );
            let (_, line_color) = tints(step.progress, theme::palette(ui.ctx()));
            label_in(
                ui,
                line_rect,
                TextRole::Small.rich(ui, &step.line).color(line_color),
            );
        }
        Density::Compact => {
            title.on_hover_text(&step.line);
        }
    }
}

fn place_marker(ui: &mut egui::Ui, rect: egui::Rect, number: usize, step: &Step) -> egui::Rect {
    let marker = egui::Rect::from_center_size(
        egui::pos2(rect.left() + HALF_STEP, rect.center().y),
        egui::Vec2::splat(size::STEP),
    );
    let state = match step.progress {
        Progress::Taken => StepState::Done,
        Progress::Waiting | Progress::NotReached => StepState::Pending,
    };
    ui.place(marker, StepMarker::new(number).state(state));
    marker
}

/// The count sits on the title's line, so the line under the title can use the whole width.
fn title_line(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    start: egui::Pos2,
    step: &Step,
) -> egui::Response {
    let line = egui::Rect::from_min_max(
        egui::pos2(rect.left(), start.y),
        egui::pos2(rect.right(), start.y + TITLE),
    );
    let (title_color, line_color) = tints(step.progress, theme::palette(ui.ctx()));
    let title_right = match &step.count {
        Some(text) => {
            let count = TextRole::Small.rich(ui, text).color(line_color);
            count_in(ui, line, count).rect.left() - space::SM
        }
        None => rect.right(),
    };
    let title_rect = egui::Rect::from_min_max(start, egui::pos2(title_right, start.y + TITLE));
    label_in(
        ui,
        title_rect,
        TextRole::Label.rich(ui, step.title).color(title_color),
    )
}

/// The colours of the title and of the line under it.
fn tints(progress: Progress, palette: &Palette) -> (egui::Color32, egui::Color32) {
    match progress {
        Progress::NotReached => (palette.text_secondary, palette.text_secondary),
        Progress::Waiting | Progress::Taken => (palette.text, palette.text_secondary),
    }
}

pub(super) fn connect(ui: &egui::Ui, upper: egui::Rect, lower: egui::Rect) {
    let x = upper.left() + HALF_STEP;
    let from = upper.center().y + HALF_STEP + space::XS;
    let to = lower.center().y - HALF_STEP - space::XS;
    if to - from >= space::XS {
        let painter = ui.painter();
        let line = theme::hairline(painter, theme::palette(painter.ctx()).hairline);
        painter.vline(x, from..=to, line);
    }
}

// A child made with new_child leaves the cursor alone. A scope would move it, and the rows
// would overlap.
fn label_in(ui: &mut egui::Ui, rect: egui::Rect, text: egui::RichText) -> egui::Response {
    let across = egui::Layout::left_to_right(egui::Align::Center);
    ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(across))
        .add(egui::Label::new(text).truncate())
}

fn count_in(ui: &mut egui::Ui, rect: egui::Rect, text: egui::RichText) -> egui::Response {
    let across = egui::Layout::right_to_left(egui::Align::Center);
    ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(across))
        .add(egui::Label::new(text))
}
