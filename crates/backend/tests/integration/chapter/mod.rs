//! Checks that the live backend reads a saved chapter from its folder on disk: its items, its
//! saved insights and its pages, with no store or model asked for the files, and that the fake
//! backend reads them the same way.

mod contents;
mod fake;
mod insights;
mod source;
