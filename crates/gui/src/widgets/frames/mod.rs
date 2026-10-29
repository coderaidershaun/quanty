//! The chrome that holds other widgets: the card around one entry, the column that gives a
//! container of text its copy button, and the sheet, header row and divider that every panel
//! starts with.

mod card;
mod copyable;
mod section;

pub use card::{Card, CardResponse};
pub use copyable::{Copyable, CopyableResponse};
pub use section::{PanelFrame, Surface, panel_frame, section_header, separator};
