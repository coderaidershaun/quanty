//! The roles that write a text from one section and nothing else: three styles of rewrite,
//! exercises with worked solutions, and a rewrite in other notation.

use crate::sample::CptKind;

use super::role::Role;

const DRAFT_SCHEMA: &str = include_str!("prompts/draft.schema.json");

/// The section as an encyclopedia article.
pub const REPHRASE_WIKI: Role = Role {
    id: "rephrase_wiki",
    version: 1,
    system_prompt: include_str!("prompts/rephrase_wiki.md"),
    schema: DRAFT_SCHEMA,
};

/// The section as a lecturer's notes.
pub const REPHRASE_LECTURE: Role = Role {
    id: "rephrase_lecture",
    version: 1,
    system_prompt: include_str!("prompts/rephrase_lecture.md"),
    schema: DRAFT_SCHEMA,
};

/// The section as questions and answers.
pub const REPHRASE_QA: Role = Role {
    id: "rephrase_qa",
    version: 1,
    system_prompt: include_str!("prompts/rephrase_qa.md"),
    schema: DRAFT_SCHEMA,
};

/// Exercises on the section, each with a worked solution.
pub const EXERCISE: Role = Role {
    id: "exercise",
    version: 1,
    system_prompt: include_str!("prompts/exercise.md"),
    schema: DRAFT_SCHEMA,
};

/// The section with its quantities named by other symbols.
pub const NOTATION: Role = Role {
    id: "notation",
    version: 1,
    system_prompt: include_str!("prompts/notation.md"),
    schema: DRAFT_SCHEMA,
};

/// The role that writes texts of this kind from a passage alone. Kinds that no such role writes
/// give `None`: the book as printed, and the explanations of relations, which are written from
/// the relation as well.
pub(super) fn writer_of(kind: CptKind) -> Option<&'static Role> {
    match kind {
        CptKind::RephraseWiki => Some(&REPHRASE_WIKI),
        CptKind::RephraseLecture => Some(&REPHRASE_LECTURE),
        CptKind::RephraseQa => Some(&REPHRASE_QA),
        CptKind::NotationVariant => Some(&NOTATION),
        CptKind::Exercise => Some(&EXERCISE),
        CptKind::Raw | CptKind::Entigraph => None,
    }
}
