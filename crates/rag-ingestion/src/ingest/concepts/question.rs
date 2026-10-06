//! What is asked about an item and what comes back: the prompt and the schema that every question
//! carries, the text that is sent for an item, and the checks that a reply must pass.

use graph::{RelationKind, UnknownRelationKind};
use serde::Deserialize;
use serde_json::Value;

use crate::ingest::items::Item;

pub(super) const SYSTEM_PROMPT: &str = include_str!("prompts/extract.md");
pub(super) const SCHEMA: &str = include_str!("prompts/extract.schema.json");

/// The model that answers the questions.
pub const EXTRACTION_MODEL: &str = "haiku";

// Raise this to ask about every item again, for example after a change to how replies are read.
// The answers that are kept under the old version are then no longer used.
pub(super) const PROMPT_VERSION: &str = "1";

/// The whole message the model reads for an item: where the item sits, then the text that is
/// embedded for it. A figure is sent as its explanation, with no picture.
pub(super) fn input_for(item: &Item) -> String {
    if item.input.title.is_empty() {
        return item.input.text.clone();
    }
    format!("{}\n\n{}", item.input.title, item.input.text)
}

/// What a good reply holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Extraction {
    pub concepts: Vec<ExtractedConcept>,
    pub relations: Vec<ExtractedRelation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExtractedConcept {
    pub name: String,
    pub definition: String,
}

/// `from` and `to` are names of concepts, written as the reply wrote them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExtractedRelation {
    pub from: String,
    pub kind: RelationKind,
    pub to: String,
}

#[derive(thiserror::Error, Debug)]
pub(super) enum ReplyError {
    #[error("the reply does not have the shape that the schema asks for")]
    Shape(#[source] serde_json::Error),

    #[error("relation {position} of the reply has a type that is not allowed")]
    UnknownRelationKind {
        position: usize,
        #[source]
        source: UnknownRelationKind,
    },
}

#[derive(Deserialize)]
struct Reply {
    concepts: Vec<ReplyConcept>,
    relations: Vec<ReplyRelation>,
}

#[derive(Deserialize)]
struct ReplyConcept {
    name: String,
    definition: String,
}

#[derive(Deserialize)]
struct ReplyRelation {
    from: String,
    #[serde(rename = "type")]
    kind: String,
    to: String,
}

impl Extraction {
    /// Reads the JSON that the model gave. A reply that does not fit is refused as a whole.
    /// Names and definitions are trimmed.
    ///
    /// # Errors
    /// - [`ReplyError::Shape`] when a field is missing or has the wrong kind of value
    /// - [`ReplyError::UnknownRelationKind`] when a relation has a type that is not one of the
    ///   fixed kinds
    pub(super) fn from_value(value: &Value) -> Result<Extraction, ReplyError> {
        let reply = Reply::deserialize(value).map_err(ReplyError::Shape)?;
        let concepts = reply
            .concepts
            .into_iter()
            .map(|concept| ExtractedConcept {
                name: concept.name.trim().to_owned(),
                definition: concept.definition.trim().to_owned(),
            })
            .collect();
        let relations =
            reply
                .relations
                .into_iter()
                .enumerate()
                .map(|(index, relation)| {
                    let kind = relation.kind.parse().map_err(|source| {
                        ReplyError::UnknownRelationKind {
                            position: index + 1,
                            source,
                        }
                    })?;
                    Ok(ExtractedRelation {
                        from: relation.from.trim().to_owned(),
                        kind,
                        to: relation.to.trim().to_owned(),
                    })
                })
                .collect::<Result<_, ReplyError>>()?;
        Ok(Extraction {
            concepts,
            relations,
        })
    }
}
