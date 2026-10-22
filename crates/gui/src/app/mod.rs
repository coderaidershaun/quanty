//! The window itself: it drains what the backend sent, draws the panels, and applies what they
//! asked for.

mod balances;
mod database_picker;
pub mod launch;
pub mod layout;
mod model_picker;
mod region;
mod shell;
mod shortcuts;
mod top_bar;

pub use shell::{App, run};
