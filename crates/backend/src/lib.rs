//! Everything that does the work the app asks for: the live backend, the fake one that answers
//! from built-in data, and the runtime that runs either of them.

mod exporting;
pub mod fake;
mod handler;
pub mod live;
mod runtime;

pub use handler::{Handler, Reply, Stop};
pub use runtime::Runtime;
