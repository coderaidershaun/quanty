//! How the form names the book of a chapter, and the books of the library that it offers.

use std::cmp::Reverse;
use std::collections::HashMap;

use crate::contract::{Catalogue, Document};

/// The book of the chapter, as the person made it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) enum BookChoice {
    /// Nothing is chosen yet.
    #[default]
    Unchosen,
    /// A book of the library, with its title exactly as the library has it.
    Existing(String),
    /// The title typed for a book that the library does not hold.
    New(String),
}

/// A book of the library that the form offers, with what its documents have in common.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Offer<'a> {
    pub(super) title: &'a str,
    pub(super) author: Option<&'a str>,
    pub(super) tags: Vec<&'a str>,
}

/// True when a book of the library has exactly this title.
pub(super) fn has_titled(catalogue: &Catalogue, title: &str) -> bool {
    catalogue
        .books
        .iter()
        .any(|book| book.title.as_deref() == Some(title))
}

/// Every book of the library that has a title, in the order of the catalogue.
pub(super) fn offers(catalogue: &Catalogue) -> Vec<Offer<'_>> {
    catalogue
        .books
        .iter()
        .filter_map(|book| {
            Some(Offer {
                title: book.title.as_deref()?,
                author: common_author(&book.chapters),
                tags: shared_tags(&book.chapters),
            })
        })
        .collect()
}

/// The author that most documents name. A tie goes to the first author in text order.
fn common_author(documents: &[Document]) -> Option<&str> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for author in documents
        .iter()
        .filter_map(|document| document.author.as_deref())
    {
        *counts.entry(author).or_default() += 1;
    }
    counts
        .into_iter()
        .min_by_key(|&(author, count)| (Reverse(count), author))
        .map(|(author, _)| author)
}

/// The tags that every document carries, in the order of the first document.
fn shared_tags(documents: &[Document]) -> Vec<&str> {
    let Some((first, others)) = documents.split_first() else {
        return Vec::new();
    };
    let mut tags: Vec<&str> = Vec::new();
    for tag in first.tags.iter().map(String::as_str) {
        let is_shared = others
            .iter()
            .all(|other| other.tags.iter().any(|t| t == tag));
        if is_shared && !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}
