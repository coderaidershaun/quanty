//! The small kit of widgets every panel is built from: buttons, inputs, tabs, cards, chips,
//! notices, a spinner and the sheets that ask a question. Each one carries an accessible label.

mod button;
mod card;
mod chip;
mod field;
pub mod gallery;
mod logo;
mod look;
mod modal;
mod notice;
mod progress;
mod section;
mod tabs;

pub use button::Button;
pub use card::{Card, CardResponse};
pub use chip::{Badge, Chip, CitationChip, StepMarker, StepState, dot};
pub use field::{Dropdown, Slider, Step, Stepper, TextInput, TextInputResponse};
pub use logo::logo;
pub use look::ControlSize;
pub use modal::{Choice, Confirm, modal};
pub use notice::{Notice, NoticeResponse, Placeholder, PlaceholderResponse};
pub use progress::{progress_bar, spinner};
pub use section::{panel_frame, section_header, separator};
pub use tabs::{Tab, TabStrip};
