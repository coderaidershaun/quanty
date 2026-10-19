//! The records of the fine-tuning files: a question, the answer that passed its checks, and the
//! proof that it passed.

use rag_core::ItemId;
use serde::{Deserialize, Serialize};

use crate::documents::Split;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SftSample {
    pub id: String,
    pub task: Task,
    pub reasoning: Reasoning,
    /// No system turn and no thinking marker: a trainer writes them with the chat template of the
    /// model it trains.
    pub messages: Vec<Message>,
    pub seed: Seed,
    pub verification: Verification,
    pub known_by_cpt_model: Option<bool>,
    pub difficulty: f64,
    pub ifd: Option<f64>,
    pub split: Split,
    pub pipeline_version: String,
}

/// `sft:<section id>:<n>`, with the number in four digits.
pub fn sft_id(section_id: &str, n: u32) -> String {
    format!("sft:{section_id}:{n:04}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    Derivation,
    Calculation,
    Explain,
    Compare,
    CodePot,
}

/// Whether the answer is written after thinking aloud or straight out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reasoning {
    Think,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Speaker,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub thinking: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    User,
    Assistant,
}

/// What a question rests on in the book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seed {
    pub kind: SeedKind,
    pub item_ids: Vec<ItemId>,
    pub section_path: Vec<String>,
    pub quote: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeedKind {
    BookWorkedExample,
    BookExercise,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verification {
    pub method: VerificationMethod,
    pub passed: bool,
    pub n_teacher_samples: u32,
    pub agreement: u32,
    pub judge_score: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    #[serde(rename = "agreement+judge")]
    AgreementAndJudge,
    PythonExec,
    Judge,
}
