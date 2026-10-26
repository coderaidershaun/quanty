//! Decides which sections are kept out of the dataset and fills in what the checks found out about
//! the ones that stay.

mod code;
mod latex;
mod page_gate;

use std::collections::HashMap;
use std::fmt;

use crate::options::BuildOptions;
use crate::sample::{Dedup, Quality};
use crate::section::Section;

/// What the checks made of one section.
#[derive(Debug, Clone)]
pub struct Checked {
    pub quality: Quality,
    pub dedup: Dedup,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Keep,
    Reject(Reason),
}

/// Why a section was kept out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// A page of the section matches too little of the text printed on it.
    PageScore,
    /// A formula of the section does not render.
    Latex,
    /// The same text after the heading line is in the section with this id, which came first.
    DuplicateOf(String),
}

impl Reason {
    /// The name that counts of rejected sections are listed by.
    pub fn name(&self) -> &'static str {
        match self {
            Reason::PageScore => "page_score",
            Reason::Latex => "latex",
            Reason::DuplicateOf(_) => "duplicate_of",
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::DuplicateOf(id) => write!(formatter, "{} {id}", self.name()),
            _ => formatter.write_str(self.name()),
        }
    }
}

/// Runs every check on the section, whatever the first one finds, so that a rejected section still
/// shows what each check made of it. The section is rejected for the first of these that applies:
/// a page that matches too little of its text layer, a formula that does not render, or the same
/// text after the heading line as a section that was kept. Only a kept section is remembered, so a
/// copy of a rejected section is judged on its own pages.
pub fn check(
    section: &Section,
    first_id_by_sha256: &mut HashMap<String, String>,
    options: &BuildOptions,
) -> Checked {
    let (prose, listings) = code::split_listings(&section.markdown);
    let quality = Quality {
        page_score: page_gate::page_score(section),
        latex_renders: latex::renders(&prose),
        code_parses: code::python_compiles(&listings),
        ..Quality::default()
    };
    let sha256 = ocr::sha256_hex(section.text_after_heading_line().as_bytes());
    let reason = if quality
        .page_score
        .is_some_and(|score| score < options.min_page_score)
    {
        Some(Reason::PageScore)
    } else if quality.latex_renders == Some(false) && !options.keep_unrendered {
        Some(Reason::Latex)
    } else {
        first_id_by_sha256
            .get(&sha256)
            .map(|first| Reason::DuplicateOf(first.clone()))
    };
    let verdict = match reason {
        Some(reason) => Verdict::Reject(reason),
        None => {
            first_id_by_sha256
                .entry(sha256.clone())
                .or_insert_with(|| section.id.clone());
            Verdict::Keep
        }
    };
    Checked {
        quality,
        dedup: Dedup {
            sha256: Some(sha256),
            minhash_cluster: None,
        },
        verdict,
    }
}
