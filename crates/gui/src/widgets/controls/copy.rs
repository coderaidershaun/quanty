//! The small icon button that copies the text of the container it stands in.

use eframe::egui::{self, Response};

use super::button::Button;
use crate::theme::Icon;
use crate::widgets::ControlSize;

const COPIED_LABEL: &str = "Copied";

/// Reads "Copied" after a click, until the pointer leaves it. A click is true on the returned
/// response, and the caller copies the text: the button only shows that it was pressed.
pub struct CopyButton<'a> {
    label: &'a str,
}

impl<'a> CopyButton<'a> {
    /// `label` is the accessible name and the hover text, such as "Copy formula".
    pub fn new(label: &'a str) -> Self {
        CopyButton { label }
    }

    /// The width and the height of the button.
    pub const fn side() -> f32 {
        ControlSize::Small.height()
    }
}

impl egui::Widget for CopyButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        // The name changes after a click, so the state is kept under the place of the button.
        // SMELL: that place is egui's automatic id, which moves when the layout before the button
        // changes, so "Copied" can show on another button for a frame. A stable id needs a key
        // from the caller.
        let id = ui.next_auto_id();
        let is_copied = ui.data(|data| data.get_temp::<bool>(id)).unwrap_or(false);
        let (icon, name) = if is_copied {
            (Icon::CHECK, COPIED_LABEL)
        } else {
            (Icon::COPY, self.label)
        };
        let response = ui.add(Button::icon_only(icon, name).size(ControlSize::Small));
        if response.clicked() {
            ui.data_mut(|data| data.insert_temp(id, true));
            ui.ctx().request_repaint();
        } else if is_copied && !ui.rect_contains_pointer(response.rect) {
            ui.data_mut(|data| data.remove::<bool>(id));
            ui.ctx().request_repaint();
        }
        response
    }
}
