//! The sheet that sits over the window, and the yes-or-no question built on it.

use eframe::egui;

use super::Button;
use crate::theme::{TextRole, color, radius, space};

/// A sheet over the window with a dimmed backdrop. Escape, a click on the backdrop, or
/// `ui.close()` closes it.
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
                .stroke(egui::Stroke::new(1.0, color::BORDER))
                .corner_radius(radius::LG)
                .inner_margin(space::XL),
        )
        .show(ctx, add_contents)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Confirmed,
    Cancelled,
}

pub struct Confirm<'a> {
    id_salt: &'a str,
    title: &'a str,
    body: Option<&'a str>,
    confirm_label: &'a str,
    cancel_label: &'a str,
    destructive: bool,
}

impl<'a> Confirm<'a> {
    pub fn new(id_salt: &'a str, title: &'a str) -> Self {
        Confirm {
            id_salt,
            title,
            body: None,
            confirm_label: "Confirm",
            cancel_label: "Cancel",
            destructive: false,
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

    /// The confirm button is dangerous: it is drawn in the danger colour.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// `None` while the person has not decided. Escape and a click outside count as cancel.
    pub fn show(self, ctx: &egui::Context) -> Option<Choice> {
        let mut choice = None;
        let sheet = modal(ctx, egui::Id::new(self.id_salt), |ui| {
            ui.set_width(360.0);
            ui.label(TextRole::Heading.rich(self.title));
            if let Some(body) = self.body {
                ui.label(TextRole::Body.rich(body).color(color::TEXT_SECONDARY));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let confirm = if self.destructive {
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
