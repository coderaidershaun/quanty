//! The pieces that ingestion and retrieval must agree on: the settings, the identifiers, the
//! embedder, the item store and the language model.

mod claude;
mod concept;
mod config;
mod embedder;
mod item;
mod item_store;
mod llm;

pub use claude::{ClaudeCli, ClaudeCliError, check_signed_in};
pub use concept::ConceptId;
pub use config::{ApiKey, Config, ConfigError};
pub use embedder::{
    DocumentInput, EMBEDDING_DIMENSIONS, EMBEDDING_MODEL, EmbedError, Embedder, Embedding,
    GeminiEmbedder,
};
pub use item::{DocId, ItemId, ItemKind, ItemPayload, ParseDocIdError, UnknownItemKind};
pub use item_store::{ItemHit, ItemPoint, ItemStore, StoreError};
pub use llm::{Llm, LlmError, Question};
