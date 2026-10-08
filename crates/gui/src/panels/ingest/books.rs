//! How the form names the book of a chapter, and the books of the library that it offers.

use crate::contract::{Catalogue, is_same_title};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) enum BookChoice {
    #[default]
    Unchosen,
    /// A book of the library, with its title exactly as the library has it.
    Existing(String),
    /// The title typed for a book that the library does not hold.
    New(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Offer<'a> {
    pub(super) title: &'a str,
    pub(super) author: Option<&'a str>,
    pub(super) tags: Vec<&'a str>,
}

/// The comparison is exact, so that a library which already holds two titles that differ only in
/// capitals still offers each of them.
pub(super) fn has_titled(catalogue: &Catalogue, title: &str) -> bool {
    catalogue
        .books
        .iter()
        .any(|book| book.title.as_deref() == Some(title))
}

/// Every titled book, also one with no chapter yet, with the labels that the library shows for it.
pub(super) fn offers(catalogue: &Catalogue) -> Vec<Offer<'_>> {
    catalogue
        .books
        .iter()
        .filter_map(|book| {
            let title = book.title.as_deref()?;
            let labels = book.labels();
            Some(Offer {
                title,
                author: labels.author,
                tags: labels.tags,
            })
        })
        .collect()
}

pub(super) fn offer_titled<'o, 'a>(offers: &'o [Offer<'a>], title: &str) -> Option<&'o Offer<'a>> {
    offers
        .iter()
        .find(|offer| is_same_title(offer.title, title))
}
