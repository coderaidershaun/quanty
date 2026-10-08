//! The author box and the tags box of a form, read as the labels a person typed.

pub(super) fn author_label(text: &str) -> Option<String> {
    let author = text.trim();
    (!author.is_empty()).then(|| author.to_owned())
}

pub(super) fn tag_labels(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}
