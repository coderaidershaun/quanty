//! The question that decides whether two concepts are one: the prompt and the schema, the message
//! that is sent, and how a reply is read.

use graph::ConceptNode;
use rag_core::{Llm, Question};
use serde::Deserialize;

use super::ask::{KeyedQuestion, Reading};
use super::cache::{Cache, key_of};
use super::question::ExtractedConcept;
use super::{ConceptError, ConceptExtractor};

const SYSTEM_PROMPT: &str = include_str!("prompts/same-concept.md");
const SCHEMA: &str = include_str!("prompts/same-concept.schema.json");

#[derive(Deserialize)]
struct Reply {
    same: bool,
}

fn input_for(new: &ExtractedConcept, stored: &ConceptNode) -> String {
    format!(
        "First concept\nname: {}\ndefinition: {}\n\nSecond concept\nname: {}\ndefinition: {}",
        new.name, new.definition, stored.name, stored.definition
    )
}

impl<L: Llm> ConceptExtractor<L> {
    /// Whether the extracted concept and the stored one are the same concept. The answer is kept
    /// in the same cache as the answers about items, so the same two concepts are asked about
    /// once.
    ///
    /// # Errors
    /// [`ConceptError::Cache`] when the cache folder cannot be used.
    pub(super) async fn same_concept(
        &self,
        cache: &Cache,
        new: &ExtractedConcept,
        stored: &ConceptNode,
    ) -> Result<Reading<bool>, ConceptError> {
        let input = input_for(new, stored);
        let question = Question {
            system_prompt: SYSTEM_PROMPT,
            schema: SCHEMA,
            input: &input,
        };
        let asked = KeyedQuestion {
            key: key_of(&self.prompt_version, self.llm.model(), question),
            question,
        };
        self.answer(cache, &asked, |value| {
            Reply::deserialize(value).map(|reply| reply.same)
        })
        .await
    }
}
