//! The sheet that sits over the window, the yes-or-no question built on it, and how a Back closes
//! the sheet or popup that is open.

use eframe::egui;

use crate::theme::{self, TextRole, radius, shadow, space, stroke};
use crate::widgets::controls::Button;

const CONFIRM_WIDTH: f32 = 360.0;

/// Where a request to close the sheet on top waits until that sheet is drawn.
fn close_request() -> egui::Id {
    egui::Id::new("close the sheet on top")
}

/// Whether a sheet or a popup is open, as the last frame left it.
pub(crate) fn is_sheet_or_popup_open(ctx: &egui::Context) -> bool {
    ctx.memory(|memory| memory.top_modal_layer().is_some()) || egui::Popup::is_any_open(ctx)
}

/// Closes what is open on top: a popup first, because it can stand over a sheet (a list of
/// choices), and else the sheet on top. A sheet closes when it is next drawn.
pub(crate) fn close_the_top_sheet(ctx: &egui::Context) {
    if egui::Popup::is_any_open(ctx) {
        egui::Popup::close_all(ctx);
    } else if let Some(layer) = ctx.memory(|memory| memory.top_modal_layer()) {
        ctx.data_mut(|data| data.insert_temp(close_request(), layer));
    }
    ctx.request_repaint();
}

/// A request that no sheet took, because none was drawn, must not close the next one.
pub(crate) fn forget_the_close_request(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<egui::LayerId>(close_request()));
}

fn take_the_close_request(ui: &egui::Ui) -> bool {
    ui.ctx().data_mut(|data| {
        let is_asked = data.get_temp::<egui::LayerId>(close_request()) == Some(ui.layer_id());
        if is_asked {
            data.remove::<egui::LayerId>(close_request());
        }
        is_asked
    })
}

/// A sheet over the window with a dimmed backdrop, `width` wide or as wide as the window leaves
/// room for. It keeps nothing: show it on every frame while it is open, and stop when
/// `should_close()` of what it returns is true. Escape, a click on the backdrop and `ui.close()`
/// make it so.
pub fn modal<R>(
    ctx: &egui::Context,
    id: egui::Id,
    width: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::ModalResponse<R> {
    let palette = theme::palette(ctx);
    let frame = egui::Frame::NONE
        .fill(palette.sheet)
        .stroke(egui::Stroke::new(stroke::BORDER, palette.border))
        .corner_radius(radius::MD)
        .inner_margin(space::XL)
        .shadow(shadow::sheet(palette));
    let sides = frame.total_margin();
    let room = ctx.content_rect().width() - sides.left - sides.right - 2.0 * space::LG;
    let sheet = egui::Modal::new(id)
        .backdrop_color(palette.scrim)
        .frame(frame)
        .show(ctx, |ui| {
            if take_the_close_request(ui) {
                ui.close();
            }
            ui.set_width(width.min(room).max(0.0));
            add_contents(ui)
        });
    // The sheet that closes now still counts as open until the next frame has run, and a Back that
    // came in between would be spent on it and lost.
    let is_escaped = sheet.is_top_modal
        && !sheet.any_popup_open
        && ctx.input(|input| input.key_pressed(egui::Key::Escape));
    if sheet.response.should_close() || sheet.backdrop_response.clicked() || is_escaped {
        ctx.request_repaint();
    }
    sheet
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

    pub fn destructive(mut self) -> Self {
        self.is_destructive = true;
        self
    }

    /// `None` while the person has not decided. Escape and a click outside count as cancel.
    pub fn show(self, ctx: &egui::Context) -> Option<Choice> {
        let mut choice = None;
        let sheet = modal(ctx, self.id, CONFIRM_WIDTH, |ui| {
            ui.label(TextRole::Heading.rich(ui, self.title));
            if let Some(body) = self.body {
                let secondary = theme::palette(ui.ctx()).text_secondary;
                ui.label(TextRole::Body.rich(ui, body).color(secondary));
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
                    ui.close();
                }
                if ui.add(Button::secondary("Cancel")).clicked() {
                    choice = Some(Choice::Cancelled);
                    ui.close();
                }
            });
        });
        if choice.is_none() && sheet.should_close() {
            choice = Some(Choice::Cancelled);
        }
        choice
    }
}
