//! Turns what is stored, and what is asked, into vectors. The wording the model expects stays
//! inside the implementation, so no caller can get it wrong.

mod gemini;
mod request;

use std::path::PathBuf;

pub use gemini::GeminiEmbedder;

use crate::usage::{ModelUsage, Usage, UsageTally};

pub const EMBEDDING_MODEL: &str = "gemini-embedding-2";
pub const EMBEDDING_DIMENSIONS: usize = 768;

pub type Embedding = Vec<f32>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentInput {
    /// Where the text sits: the book, the chapter and the section. Empty when it has no place.
    pub title: String,
    pub text: String,
    /// A PNG or JPEG that is embedded together with the text, as one vector.
    pub image: Option<PathBuf>,
}

/// Stored items and questions are worded differently, so that a question lands near the documents
/// that answer it.
pub trait Embedder {
    /// One vector for each input, in the order given.
    ///
    /// # Errors
    /// When any input fails, the whole call fails: nothing is returned for a partly embedded
    /// list.
    fn embed_document(
        &self,
        inputs: &[DocumentInput],
    ) -> impl Future<Output = Result<Vec<Embedding>, EmbedError>> + Send;

    fn embed_query(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Embedding, EmbedError>> + Send;
}

/// What embedding these inputs uses, worked out from the length of the text that is sent, because
/// the embeddings API gives no count: one token for every four characters, rounded up. A picture
/// is counted by its text alone.
pub fn embedding_usage(inputs: &[DocumentInput]) -> UsageTally {
    let tokens = inputs
        .iter()
        .map(|input| tokens_of(&request::document_text(input)))
        .sum();
    estimated(tokens)
}

/// What embedding a question uses, worked out as [`embedding_usage`] works it out.
pub fn question_usage(question: &str) -> UsageTally {
    estimated(tokens_of(&request::query_text(question)))
}

fn tokens_of(text: &str) -> u64 {
    text.chars().count().div_ceil(4) as u64
}

fn estimated(input_tokens: u64) -> UsageTally {
    let usage = ModelUsage {
        tokens: Usage {
            input_tokens,
            ..Usage::default()
        },
        reported_usd: None,
        estimated: true,
    };
    UsageTally::of(EMBEDDING_MODEL, usage)
}

#[derive(thiserror::Error, Debug)]
pub enum EmbedError {
    #[error("EMBEDDING_GEMINI_API_KEY is not set; add it to .env or to the environment")]
    MissingApiKey,

    #[error("could not read the picture {}", path.display())]
    Image {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("the picture {} is not a PNG or a JPEG, judging by its file name", path.display())]
    UnsupportedImage { path: PathBuf },

    #[error("the request to the embeddings API failed")]
    Transport(#[from] reqwest::Error),

    #[error("the embeddings API answered {status}: {body}")]
    Rejected { status: u16, body: String },

    #[error("the embeddings API sent a reply that is not the expected shape: {body}")]
    UnreadableReply {
        body: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("asked for {expected} vectors but the embeddings API sent {got}")]
    WrongVectorCount { expected: usize, got: usize },

    #[error("expected vectors of {expected} numbers but the embeddings API sent one of {got}")]
    WrongVectorLength { expected: usize, got: usize },
}
