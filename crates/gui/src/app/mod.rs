//! The window itself: it drains what the backend sent, draws the panels, and applies what they
//! asked for.

mod ask_nav;
mod balances;
mod footer;
pub mod layout;
mod model_picker;
mod region;
mod shell;
mod shortcuts;
mod top_bar;

pub use shell::App;
