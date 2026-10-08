//! The author box and the tags box of a form: their names, the caption over a box, and the labels
//! to and from the text a person types.

use std::borrow::Borrow;

use eframe::egui;

use crate::theme::{TextRole, space};

pub(super) const AUTHOR: &str = "Author";
pub(super) const TAGS: &str = "Tags";

pub(super) fn caption(ui: &mut egui::Ui, text: &str) {
    ui.add_space(space::SM);
    ui.label(TextRole::Label.rich(text));
}

pub(super) fn author_label(text: &str) -> Option<String> {
    let author = text.trim();
    (!author.is_empty()).then(|| author.to_owned())
}

pub(super) fn author_text(author: Option<&str>) -> String {
    author.unwrap_or_default().to_owned()
}

pub(super) fn tag_labels(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(super) fn tags_text<S: Borrow<str>>(tags: &[S]) -> String {
    tags.join(", ")
}
