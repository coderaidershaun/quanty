//! The settings every program of this project reads, from the environment and the `.env` file.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

const QDRANT_URL: &str = "QDRANT_URL";
const FALKORDB_URL: &str = "FALKORDB_URL";
const FALKORDB_GRAPH: &str = "FALKORDB_GRAPH";
const QDRANT_ITEMS_COLLECTION: &str = "QDRANT_ITEMS_COLLECTION";
const GEMINI_API_KEY: &str = "EMBEDDING_GEMINI_API_KEY";
const QDRANT_CONCEPTS_COLLECTION: &str = "QDRANT_CONCEPTS_COLLECTION";
const CONCEPT_CACHE_DIR: &str = "CONCEPT_CACHE_DIR";
const CONCEPT_DECISION_LOG: &str = "CONCEPT_DECISION_LOG";
const CONTENT_DIR: &str = "CONTENT_DIR";

const DEFAULT_QDRANT_URL: &str = "http://localhost:6334";
const DEFAULT_FALKORDB_URL: &str = "falkor://localhost:6379";
const DEFAULT_FALKORDB_GRAPH: &str = "quanty";
const DEFAULT_ITEMS_COLLECTION: &str = "items";
const DEFAULT_CONCEPTS_COLLECTION: &str = "concepts";
const DEFAULT_CONCEPT_CACHE_FOLDER: &str = "data/concept-cache";
const DEFAULT_CONCEPT_DECISION_LOG: &str = "data/concept-decisions.jsonl";
const DEFAULT_CONTENT_FOLDER: &str = "content";

const DOTENV_FILE_NAME: &str = ".env";

/// A secret that shows nothing of itself when printed with `{:?}`, so a config can be logged.
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// The key itself, for the one place that sends it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiKey(hidden)")
    }
}

/// Where the services are and which collection to use. A setting that nobody set has its local
/// default.
#[derive(Debug, Clone)]
pub struct Config {
    /// The gRPC address of Qdrant.
    pub qdrant_url: String,
    pub falkordb_url: String,
    /// The graph that documents and items are written to. Tests set another name so they never
    /// touch the real graph.
    pub falkordb_graph: String,
    /// Where items are stored. Tests set another name so they never touch the real collection.
    pub items_collection: String,
    /// Where the vectors of concepts are stored. Tests set another name so they never touch the
    /// real collection.
    pub concepts_collection: String,
    /// Where the answers of concept extraction are kept, so that the same question is never paid
    /// for twice. A relative path starts at the folder the command runs in. Tests set a temporary
    /// folder so they never touch the real one.
    pub concept_cache_folder: PathBuf,
    /// The file that gets one line for each decision about which stored concept a name belongs
    /// to. A relative path starts at the folder the command runs in. Tests set a temporary file so
    /// they never touch the real one.
    pub concept_decision_log: PathBuf,
    /// Where the converted files of a picture that stands alone are saved. A relative path starts
    /// at the folder the command runs in. Tests set a temporary folder so they never touch the
    /// real one.
    pub content_folder: PathBuf,
    /// Not needed by every command, so a missing key is not an error here.
    pub gemini_api_key: Option<ApiKey>,
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("could not find the current folder to look for a .env file in")]
    CurrentFolder(#[source] std::io::Error),

    #[error("could not read {}", path.display())]
    DotenvRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    // The bad line is left out on purpose: it may be a key.
    #[error("{} is not a list of NAME=value lines", path.display())]
    DotenvParse { path: PathBuf },
}

impl Config {
    /// Reads the process environment, then the `.env` found in the current folder or the nearest
    /// parent that has one, then the local defaults. The process environment is never changed.
    ///
    /// # Errors
    /// - [`ConfigError::CurrentFolder`] when the current folder cannot be found
    /// - [`ConfigError::DotenvRead`] and [`ConfigError::DotenvParse`] for a `.env` that is
    ///   unreadable or not a list of `NAME=value` lines
    pub fn load() -> Result<Config, ConfigError> {
        let current_folder = std::env::current_dir().map_err(ConfigError::CurrentFolder)?;
        let dotenv_file = current_folder
            .ancestors()
            .map(|folder| folder.join(DOTENV_FILE_NAME))
            .find(|candidate| candidate.is_file());
        Config::from_sources(|name| std::env::var(name).ok(), dotenv_file.as_deref())
    }

    /// The rules of [`Config::load`] with both sources given, so a caller controls them. The
    /// environment wins over the file. A value that is empty or only spaces counts as unset, and
    /// so does a file that does not exist.
    ///
    /// # Errors
    /// - [`ConfigError::DotenvRead`] when the file exists but cannot be read
    /// - [`ConfigError::DotenvParse`] when the file is not a list of `NAME=value` lines
    pub fn from_sources(
        environment: impl Fn(&str) -> Option<String>,
        dotenv_file: Option<&Path>,
    ) -> Result<Config, ConfigError> {
        let dotenv = match dotenv_file {
            Some(path) => read_dotenv(path)?,
            None => HashMap::new(),
        };
        let setting = |name: &str| {
            non_blank(environment(name)).or_else(|| non_blank(dotenv.get(name).cloned()))
        };
        Ok(Config {
            qdrant_url: setting(QDRANT_URL).unwrap_or_else(|| DEFAULT_QDRANT_URL.to_owned()),
            falkordb_url: setting(FALKORDB_URL).unwrap_or_else(|| DEFAULT_FALKORDB_URL.to_owned()),
            falkordb_graph: setting(FALKORDB_GRAPH)
                .unwrap_or_else(|| DEFAULT_FALKORDB_GRAPH.to_owned()),
            items_collection: setting(QDRANT_ITEMS_COLLECTION)
                .unwrap_or_else(|| DEFAULT_ITEMS_COLLECTION.to_owned()),
            concepts_collection: setting(QDRANT_CONCEPTS_COLLECTION)
                .unwrap_or_else(|| DEFAULT_CONCEPTS_COLLECTION.to_owned()),
            concept_cache_folder: setting(CONCEPT_CACHE_DIR)
                .unwrap_or_else(|| DEFAULT_CONCEPT_CACHE_FOLDER.to_owned())
                .into(),
            concept_decision_log: setting(CONCEPT_DECISION_LOG)
                .unwrap_or_else(|| DEFAULT_CONCEPT_DECISION_LOG.to_owned())
                .into(),
            content_folder: setting(CONTENT_DIR)
                .unwrap_or_else(|| DEFAULT_CONTENT_FOLDER.to_owned())
                .into(),
            gemini_api_key: setting(GEMINI_API_KEY).map(ApiKey::new),
        })
    }
}

fn non_blank(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The first value of a name wins, as in the `dotenvy` loaders.
fn read_dotenv(path: &Path) -> Result<HashMap<String, String>, ConfigError> {
    let lines = match dotenvy::from_path_iter(path) {
        Ok(lines) => lines,
        Err(error) if error.not_found() => return Ok(HashMap::new()),
        Err(error) => return Err(dotenv_error(path, error)),
    };
    let mut values = HashMap::new();
    for line in lines {
        let (name, value) = line.map_err(|error| dotenv_error(path, error))?;
        values.entry(name).or_insert(value);
    }
    Ok(values)
}

/// Not `#[from]`: the `dotenvy` parse error prints the whole bad line, which may be a key.
fn dotenv_error(path: &Path, error: dotenvy::Error) -> ConfigError {
    match error {
        dotenvy::Error::Io(source) => ConfigError::DotenvRead {
            path: path.to_path_buf(),
            source,
        },
        _ => ConfigError::DotenvParse {
            path: path.to_path_buf(),
        },
    }
}
