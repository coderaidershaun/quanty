//! The pieces that ingestion and retrieval must agree on: the settings, the identifiers, the
//! embedder, the item store, the concept store, the language model and the prices of the models.

mod concept;
mod config;
mod content;
mod embedder;
mod error_chain;
mod item;
mod labels;
mod llm;
mod qdrant;
mod usage;

pub use choices::{ApiKey, ModelSettings, SettingsError, WorkKind};
pub use concept::ConceptId;
pub use config::{Config, ConfigError};
pub use content::content_url;
pub use embedder::{
    DocumentInput, EMBEDDING_DIMENSIONS, EMBEDDING_MODEL, EmbedError, Embedder, Embedding,
    GeminiEmbedder, embedding_usage, question_usage,
};
pub use error_chain::error_chain;
pub use item::{DocId, ItemId, ItemKind, ItemPayload, ParseDocIdError, UnknownItemKind};
pub use labels::{
    Category, DocumentLabels, EmptyTag, LabelFilter, MediaLabels, Tag, UnknownCategory,
    WantedDocument, author_list, is_same_name,
};
pub use llm::{
    AgentCli, ClaudeCli, ClaudeCliError, CodexCli, CodexError, Llm, LlmError, LlmReply, Question,
};
pub use qdrant::{
    ConceptHit, ConceptPoint, ConceptStore, ItemFilter, ItemHit, ItemPoint, ItemStore, StoreError,
    concept_input,
};
pub use usage::{ModelPrice, ModelUsage, Usage, UsageTally, cost_text, price_of};
