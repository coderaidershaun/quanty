//! The sizes that controls come in.

use crate::theme::size;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}
