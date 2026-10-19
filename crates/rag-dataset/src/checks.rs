//! Decides which sections are kept out of the dataset and fills in what the checks found out about
//! the ones that stay. Nothing is checked yet: every section is kept.

use std::fmt;
use std::sync::Once;

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
    /// The same text is in the section with this id, which came first.
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

/// What the checks have seen of the sections so far, so that a copy of a section is found.
#[derive(Debug, Default)]
pub struct Seen {}

/// Not built yet: keeps every section and leaves every finding null.
pub fn check(_section: &Section, _seen: &mut Seen, _options: &BuildOptions) -> Checked {
    static SAID_SO: Once = Once::new();
    SAID_SO.call_once(|| {
        tracing::info!("the checks are not built yet, so every section is kept with no findings");
    });
    Checked {
        quality: Quality::default(),
        dedup: Dedup::default(),
        verdict: Verdict::Keep,
    }
}
