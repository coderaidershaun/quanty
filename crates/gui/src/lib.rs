//! The desktop app: one window where a person asks the books a question and reads the answer
//! beside the page it stands on. It draws as paper and ink: a pale desk, sheets of paper, a
//! darker well under the scanned page, Literata for what is read, Source Sans 3 for controls, and
//! the rubric red only for the thread from a citation to its source.

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
