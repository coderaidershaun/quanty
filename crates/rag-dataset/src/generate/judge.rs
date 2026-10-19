//! The role that reads a draft against the passage it was written from, and the rule that decides
//! whether the draft is kept.

use std::ops::RangeInclusive;

use serde::Deserialize;

use crate::sample::CptKind;

use super::role::Role;

/// A check of one text against its source, claim by claim.
pub const JUDGE: Role = Role {
    id: "judge",
    version: 1,
    system_prompt: include_str!("prompts/judge.md"),
    schema: include_str!("prompts/judge.schema.json"),
};

/// The scores of a text worth training on: the judge's middle mark and above.
const KEPT_SCORES: RangeInclusive<u8> = 3..=5;

/// What the judge answers about one text.
#[derive(Debug, Deserialize)]
pub(super) struct Verdict {
    pub(super) unsupported_claims: Vec<String>,
    faithful: bool,
    pub(super) score: u8,
}

impl Verdict {
    /// Faithful, with no claim the source does not support, and a score worth training on.
    pub(super) fn passes(&self) -> bool {
        self.faithful && self.unsupported_claims.is_empty() && KEPT_SCORES.contains(&self.score)
    }
}

/// What the judge is asked: the kind of the text, the passage it was written from, and the text.
/// The kind tells the judge what that kind of text may add to its source.
pub(super) fn input(kind: CptKind, passage: &str, draft: &str) -> String {
    format!(
        "Kind: {}\n\n{passage}\n\n<candidate>\n{draft}\n</candidate>",
        kind.as_str()
    )
}
