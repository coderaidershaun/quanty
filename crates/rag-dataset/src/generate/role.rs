//! One kind of job for the model: the prompt that sets it, the shape its answer must have, and the
//! version that keeps the answers of an old prompt apart from the answers of a new one.

use rag_core::Question;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Role {
    /// Short and lower case, such as `rephrase_wiki`.
    pub id: &'static str,
    /// Raised whenever the prompt or the schema changes, so that old answers are not reused.
    pub version: u32,
    pub system_prompt: &'static str,
    /// A JSON Schema, as JSON text.
    pub schema: &'static str,
}

impl Role {
    /// The role and its version, such as `rephrase_wiki@1`. It names the prompt in the files.
    pub fn prompt_id(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }

    pub fn question<'a>(&'a self, input: &'a str) -> Question<'a> {
        Question {
            system_prompt: self.system_prompt,
            schema: self.schema,
            input,
        }
    }
}
