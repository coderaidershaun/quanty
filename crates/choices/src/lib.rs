//! The model choices a person saves for each kind of work, which every program reads.

mod api_key;
mod models;

pub use api_key::ApiKey;
pub use models::{
    DEFAULT_CODEX_MODEL, MODEL_CHOICES, MODEL_SETTINGS_FILE, ModelSettings, SettingsError,
    THINKING_LEVELS, WorkChoices, WorkKind,
};
