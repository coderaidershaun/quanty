//! How the form names the media of a PDF, a media of the library or a new one that is typed and
//! saved first, and which media of the library the form offers.

use crate::contract::{Catalogue, Category, Media, NewMedia};
use crate::panels::labels::list_of;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) enum MediaChoice {
    #[default]
    Unchosen,
    /// A media of the library, with its title exactly as the library has it.
    Existing { title: String, category: Category },
    /// A media that is typed, which is saved before a PDF of it can be ingested.
    New(NewMediaForm),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct NewMediaForm {
    pub(super) category: Category,
    pub(super) title: String,
    pub(super) authors: String,
    pub(super) tags: String,
}

impl NewMediaForm {
    pub(super) fn to_save(&self) -> Option<NewMedia> {
        let title = self.title.trim();
        (!title.is_empty()).then(|| NewMedia {
            title: title.to_owned(),
            category: self.category,
            authors: list_of(&self.authors),
            tags: list_of(&self.tags),
        })
    }
}

/// The comparison is exact, so that a library which already holds two titles that differ only in
/// capitals still offers each of them.
pub(super) fn titled<'a>(catalogue: &'a Catalogue, title: &str) -> Option<&'a Media> {
    catalogue
        .media
        .iter()
        .find(|media| media.title.as_deref() == Some(title))
}

/// Every titled media, also one with no document yet.
pub(super) fn offers(catalogue: &Catalogue) -> Vec<&Media> {
    catalogue
        .media
        .iter()
        .filter(|media| media.title.is_some())
        .collect()
}
