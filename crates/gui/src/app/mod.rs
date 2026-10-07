//! The window itself: it drains what the backend sent, draws the panels, and applies what they
//! asked for.

pub mod launch;
pub mod layout;
mod region;
mod shell;
mod shortcuts;
mod top_bar;

pub use shell::{App, run};
