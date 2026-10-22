//! The choices a person saves in the quanty home folder, which every program reads: the model of
//! each kind of work and the database that holds the library.

mod api_key;
mod databases;
mod models;

pub use api_key::ApiKey;
pub use databases::{Chosen, DATABASES_FILE, Database, Databases, DatabasesError, LOCAL_DATABASE};
pub use models::{
    DEFAULT_CODEX_MODEL, MODEL_CHOICES, MODEL_SETTINGS_FILE, ModelSettings, SettingsError,
    THINKING_LEVELS, WorkChoices, WorkKind,
};
