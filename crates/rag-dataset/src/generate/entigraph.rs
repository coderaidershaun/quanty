//! The role that explains how two concepts are related, what it is asked, and which passage of the
//! section it reads.

use crate::plan::RelationSeed;

use super::passage::body_of;
use super::role::Role;

/// An explanation of one relation between two concepts, from the section that states it.
pub const ENTIGRAPH: Role = Role {
    id: "entigraph",
    version: 1,
    system_prompt: include_str!("prompts/entigraph.md"),
    schema: include_str!("prompts/draft.schema.json"),
};

/// The relation as a sentence, each concept with its definition, and then the passage.
pub(super) fn input(relation: &RelationSeed, passage: &str) -> String {
    let RelationSeed { kind, from, to, .. } = relation;
    let sentence = format!("{} {kind} {}", from.name, to.name);
    let definitions = [from, to]
        .map(|concept| format!("{}: {}", concept.name, concept.definition))
        .join("\n");
    format!("<relation>\n{sentence}\n{definitions}\n</relation>\n\n{passage}")
}

/// The first passage whose own text names the most of the relation's two concepts, ignoring case.
/// The notes are left out of the count, because every passage of a section repeats them. With no
/// name in any passage it is the first passage, and with no passage it is an empty text.
pub(super) fn passage_for<'p>(relation: &RelationSeed, passages: &'p [String]) -> &'p str {
    let names = [
        relation.from.name.to_lowercase(),
        relation.to.name.to_lowercase(),
    ];
    let names_in = |passage: &str| {
        let body = body_of(passage).to_lowercase();
        names
            .iter()
            .filter(|name| body.contains(name.as_str()))
            .count()
    };
    let mut best: Option<(&String, usize)> = None;
    for passage in passages {
        let count = names_in(passage);
        if best.is_none_or(|(_, most)| count > most) {
            best = Some((passage, count));
        }
    }
    best.map_or("", |(passage, _)| passage.as_str())
}
