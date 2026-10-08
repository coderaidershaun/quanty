//! The sheet that sits over the window, and the yes-or-no question built on it.

use eframe::egui;

use super::Button;
use crate::theme::{TextRole, color, radius, space, stroke};

const CONFIRM_WIDTH: f32 = 360.0;

/// A sheet over the window with a dimmed backdrop. It keeps nothing: show it on every frame
/// while it is open, and stop when `should_close()` of what it returns is true. Escape, a click
/// on the backdrop and `ui.close()` make it so.
pub fn modal<R>(
    ctx: &egui::Context,
    id: egui::Id,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::ModalResponse<R> {
    egui::Modal::new(id)
        .backdrop_color(color::SCRIM)
        .frame(
            egui::Frame::NONE
                .fill(color::RAISED)
                .stroke(egui::Stroke::new(stroke::BORDER, color::BORDER))
                .corner_radius(radius::LG)
                .inner_margin(space::XL)
                .shadow(ctx.global_style().visuals.popup_shadow),
        )
        .show(ctx, add_contents)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Choice {
    Confirmed,
    Cancelled,
}

/// A yes-or-no question over the window. Call `show` on every frame until it answers.
pub struct Confirm<'a> {
    id: egui::Id,
    title: &'a str,
    body: Option<&'a str>,
    confirm_label: &'a str,
    cancel_label: &'a str,
    is_destructive: bool,
}

impl<'a> Confirm<'a> {
    /// `id` must be different for each sheet of the window. `title` is the question.
    pub fn new(id: egui::Id, title: &'a str) -> Self {
        Confirm {
            id,
            title,
            body: None,
            confirm_label: "Confirm",
            cancel_label: "Cancel",
            is_destructive: false,
        }
    }

    pub fn body(mut self, body: &'a str) -> Self {
        self.body = Some(body);
        self
    }

    pub fn confirm_label(mut self, label: &'a str) -> Self {
        self.confirm_label = label;
        self
    }

    pub fn cancel_label(mut self, label: &'a str) -> Self {
        self.cancel_label = label;
        self
    }

    pub fn destructive(mut self) -> Self {
        self.is_destructive = true;
        self
    }

    /// `None` while the person has not decided. Escape and a click outside count as cancel.
    pub fn show(self, ctx: &egui::Context) -> Option<Choice> {
        let mut choice = None;
        let sheet = modal(ctx, self.id, |ui| {
            ui.set_width(CONFIRM_WIDTH);
            ui.label(TextRole::Heading.rich(self.title));
            if let Some(body) = self.body {
                ui.label(TextRole::Body.rich(body).color(color::TEXT_SECONDARY));
            }
            ui.add_space(space::MD);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let confirm = if self.is_destructive {
                    Button::danger(self.confirm_label)
                } else {
                    Button::primary(self.confirm_label)
                };
                if ui.add(confirm).clicked() {
                    choice = Some(Choice::Confirmed);
                }
                if ui.add(Button::ghost(self.cancel_label)).clicked() {
                    choice = Some(Choice::Cancelled);
                }
            });
        });
        if choice.is_none() && sheet.should_close() {
            choice = Some(Choice::Cancelled);
        }
        choice
    }
}
