//! Everything that does the work the window asks for: the live backend, the fake one that answers
//! from built-in data, and the runtime that runs either off the window's thread.

pub mod fake;
mod handler;
pub mod live;
mod runtime;

pub use handler::{Handler, Reply, Stop};
pub use runtime::Backend;
