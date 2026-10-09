//! The pieces that ingestion and retrieval must agree on: the settings, the identifiers, the
//! embedder, the item store, the concept store, the language model and the prices of the models.

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
mod usage;

pub use claude::{ClaudeCli, ClaudeCliError, check_signed_in};
pub use concept::ConceptId;
pub use concept_store::{ConceptHit, ConceptPoint, ConceptStore, concept_input};
pub use config::{ApiKey, Config, ConfigError};
pub use embedder::{
    DocumentInput, EMBEDDING_DIMENSIONS, EMBEDDING_MODEL, EmbedError, Embedder, Embedding,
    GeminiEmbedder, embedding_usage, question_usage,
};
pub use item::{DocId, ItemId, ItemKind, ItemPayload, ParseDocIdError, UnknownItemKind};
pub use item_store::{ItemFilter, ItemHit, ItemPoint, ItemStore};
pub use labels::{
    Category, DocumentLabels, EmptyTag, LabelFilter, MediaLabels, Tag, UnknownCategory,
    author_list, is_same_name,
};
pub use llm::{Llm, LlmError, LlmReply, Question};
pub use qdrant::StoreError;
pub use usage::{ModelPrice, ModelUsage, Usage, UsageTally, cost_text, price_of};
