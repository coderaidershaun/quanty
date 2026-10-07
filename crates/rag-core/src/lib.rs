//! The pieces that ingestion and retrieval must agree on: the settings, the identifiers, the
//! embedder, the item store, the concept store and the language model.

mod claude;
mod concept;
mod concept_store;
mod config;
mod embedder;
mod item;
mod item_store;
mod labels;
mod llm;
mod qdrant;

pub use claude::{ClaudeCli, ClaudeCliError, check_signed_in};
pub use concept::ConceptId;
pub use concept_store::{ConceptHit, ConceptPoint, ConceptStore, concept_input};
pub use config::{ApiKey, Config, ConfigError};
pub use embedder::{
    DocumentInput, EMBEDDING_DIMENSIONS, EMBEDDING_MODEL, EmbedError, Embedder, Embedding,
    GeminiEmbedder,
};
pub use item::{DocId, ItemId, ItemKind, ItemPayload, ParseDocIdError, UnknownItemKind};
pub use item_store::{ItemFilter, ItemHit, ItemPoint, ItemStore};
pub use labels::{DocumentLabels, EmptyTag, Tag};
pub use llm::{Llm, LlmError, Question};
pub use qdrant::StoreError;
