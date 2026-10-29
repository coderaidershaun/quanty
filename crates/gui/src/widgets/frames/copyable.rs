//! Puts a copy button at the top right of a container of text, in a column of its own, so that
//! the button never lies over the text.

use eframe::egui::{self, Response, pos2, vec2};

use crate::theme::space;
use crate::widgets::controls::CopyButton;

pub struct Copyable<'a> {
    label: &'a str,
}

pub struct CopyableResponse<R> {
    pub inner: R,
    /// The response of the container, without the button.
    pub response: Response,
    /// True in the frame the button was clicked. The caller copies the text.
    pub copied: bool,
}

impl<'a> Copyable<'a> {
    /// `label` is the accessible name of the button, such as "Copy formula".
    pub fn new(label: &'a str) -> Self {
        Copyable { label }
    }

    /// The room the button takes at the right of the contents.
    pub const fn gutter() -> f32 {
        CopyButton::side() + space::XS
    }

    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> CopyableResponse<R> {
        let top_left = ui.next_widget_position();
        let width = ui.available_width();
        let room = egui::Rect::from_min_size(
            top_left,
            vec2(
                if width.is_finite() {
                    (width - Self::gutter()).max(0.0)
                } else {
                    width
                },
                ui.available_height(),
            ),
        );
        let inner = ui.scope_builder(egui::UiBuilder::new().max_rect(room), add_contents);
        let right = if width.is_finite() {
            top_left.x + width
        } else {
            inner.response.rect.right() + Self::gutter()
        };
        let side = CopyButton::side();
        let at = egui::Rect::from_min_size(pos2(right - side, top_left.y), vec2(side, side));
        // `Ui::put` would move the cursor back up to the button, so the button gets a child of
        // its own, and the cursor only makes room for it when the contents are shorter.
        let copied = ui
            .new_child(egui::UiBuilder::new().max_rect(at))
            .add(CopyButton::new(self.label))
            .clicked();
        if inner.response.rect.height() < side {
            ui.advance_cursor_after_rect(at);
        }
        CopyableResponse {
            inner: inner.inner,
            response: inner.response,
            copied,
        }
    }
}
