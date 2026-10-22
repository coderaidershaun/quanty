//! Everything the live backend reads from the chapter folder of a stored document: the folder
//! itself, its items, its saved insights and its pages.

mod contents;
mod folder;
mod insights;
pub mod source;

pub(super) use contents::load_chapter_contents;
pub(super) use folder::chapters_on_disk;
pub(super) use insights::load_chapter_insights;
