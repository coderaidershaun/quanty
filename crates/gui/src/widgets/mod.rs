//! The small kit of widgets every panel is built from: buttons, inputs, tabs, rows of a list,
//! cards, chips, notices, a spinner and the sheets that ask a question. Each one carries an
//! accessible label.

// SMELL: thirteen modules are more than one folder should hold. The gallery already sorts them
// into controls, frames, marks and messages, and the folder could follow the same groups.
mod button;
mod card;
mod chip;
mod field;
pub mod gallery;
mod list_row;
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
pub use field::{Dropdown, Slider, Step, Stepper, TextInput, TextInputResponse, paint_caret};
pub use list_row::ListRow;
pub use logo::logo;
pub use look::ControlSize;
pub use modal::{Choice, Confirm, modal};
pub use notice::{Notice, NoticeResponse, Placeholder, PlaceholderResponse};
pub use progress::{indeterminate_bar, progress_bar, spinner};
pub use section::{PanelFrame, Surface, panel_frame, section_header, separator};
pub use tabs::{Tab, TabStrip};
