//! The pieces that ingestion and retrieval must agree on: the settings, the item identifiers,
//! the embedder and the item store.

mod claude;
mod config;
mod embedder;
mod item;
mod item_store;

pub use claude::{ClaudeCliError, check_signed_in};
pub use config::{ApiKey, Config, ConfigError};
pub use embedder::{
    DocumentInput, EMBEDDING_DIMENSIONS, EMBEDDING_MODEL, EmbedError, Embedder, Embedding,
    GeminiEmbedder,
};
pub use item::{DocId, ItemId, ItemKind, ItemPayload, ParseDocIdError};
pub use item_store::{ItemPoint, ItemStore, StoreError};
