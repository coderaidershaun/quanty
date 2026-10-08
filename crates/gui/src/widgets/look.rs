//! What the hand-painted widgets share: the sizes that controls come in, the state a pointer or
//! the keyboard puts a widget in, and the ring that shows the keyboard focus.

use eframe::egui::{self, Rect, Response};

use crate::theme::{color, size, stroke};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlSize {
    Small,
    Medium,
    Large,
}

impl ControlSize {
    /// Points.
    pub const fn height(self) -> f32 {
        match self {
            ControlSize::Small => size::CONTROL_SM,
            ControlSize::Medium => size::CONTROL_MD,
            ControlSize::Large => size::CONTROL_LG,
        }
    }

    /// Points. The icon that sits in a control of this size.
    pub(super) const fn icon(self) -> f32 {
        match self {
            ControlSize::Small | ControlSize::Medium => size::ICON_MD,
            ControlSize::Large => size::ICON_LG,
        }
    }
}

/// What is being done to a widget. The gallery forces one, so a state can be seen with no
/// pointer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Look {
    pub(super) hovered: bool,
    pub(super) pressed: bool,
    pub(super) focused: bool,
}

impl Look {
    pub(super) const HOVERED: Look = Look {
        hovered: true,
        pressed: false,
        focused: false,
    };
    pub(super) const PRESSED: Look = Look {
        hovered: true,
        pressed: true,
        focused: false,
    };
    pub(super) const FOCUSED: Look = Look {
        hovered: false,
        pressed: false,
        focused: true,
    };

    pub(super) fn of(response: &Response) -> Look {
        Look {
            hovered: response.hovered(),
            pressed: response.is_pointer_button_down_on(),
            focused: response.has_focus(),
        }
    }
}

/// The keyboard focus ring: drawn just outside `rect`, so it never changes the size of a widget.
pub(super) fn focus_ring(painter: &egui::Painter, rect: Rect, corner_radius: f32) {
    painter.rect_stroke(
        rect,
        corner_radius,
        egui::Stroke::new(stroke::FOCUS, color::FOCUS),
        egui::StrokeKind::Outside,
    );
}
