//! The desktop app: one window where a person asks the books a question and reads the answer
//! beside the page it stands on.

#![recursion_limit = "256"]

pub mod app;
pub mod backend;
pub mod contract;
pub mod media;
pub mod panels;
pub mod state;
#[cfg(feature = "testing")]
pub mod testkit;
pub mod theme;
pub mod widgets;
