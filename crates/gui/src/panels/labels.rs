//! The authors box and the tags boxes of a form: their names, the caption over a box, and the
//! lists to and from the text a person types. Also the line that shows a media's category and
//! authors.

use std::borrow::Borrow;

use eframe::egui;

use crate::contract::Media;
use crate::theme::{TextRole, space};

pub(super) const AUTHORS: &str = "Authors";
pub(super) const TAGS: &str = "Tags";
pub(super) const LIST_PLACEHOLDER: &str = "Optional, with commas between them";

pub(super) fn caption(ui: &mut egui::Ui, text: &str) {
    ui.add_space(space::SM);
    ui.label(TextRole::Label.rich(text));
}

/// One rule for every list a person types, authors and tags alike: commas between the items,
/// and a blank item is dropped.
pub(super) fn list_of(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(super) fn text_of<S: Borrow<str>>(items: &[S]) -> String {
    items.join(", ")
}

pub(super) fn category_and_authors(media: &Media) -> String {
    let mut line = media.category.label().to_owned();
    if !media.authors.is_empty() {
        line.push_str(" · ");
        line.push_str(&media.authors.join(", "));
    }
    line
}
