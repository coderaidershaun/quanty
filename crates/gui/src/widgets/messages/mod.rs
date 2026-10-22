//! What the app says to the person: the strip above content, the message that fills an empty
//! area, and the sheet that asks a question.

mod modal;
mod notice;

pub use modal::{Choice, Confirm, modal};
pub use notice::{Notice, NoticeResponse, Placeholder, PlaceholderResponse};
