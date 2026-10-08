//! How the form names the book of a chapter, and the books of the library that it offers.

use std::cmp::Reverse;
use std::collections::HashMap;

use crate::contract::{Catalogue, Document};

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

pub(super) fn has_titled(catalogue: &Catalogue, title: &str) -> bool {
    catalogue
        .books
        .iter()
        .any(|book| book.title.as_deref() == Some(title))
}

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
