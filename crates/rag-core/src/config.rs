//! The settings every program reads from the environment and the `.env` file.

use crate::ApiKey;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const DOTENV_FILE_NAME: &str = ".env";

/// A setting that nobody set has its local default.
#[derive(Debug, Clone)]
pub struct Config {
    /// The gRPC address of Qdrant.
    pub qdrant_url: String,
    /// Not needed by a Qdrant that has no key set.
    pub qdrant_api_key: Option<ApiKey>,
    pub falkordb_url: String,
    /// Not needed by a FalkorDB that has no password set.
    pub falkordb_password: Option<ApiKey>,
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
    /// Where the converted files of a picture that stands alone, and of the chapters that
    /// `rag-ingest pdf` converts, are saved. A relative path starts at the folder the command runs
    /// in. Tests set a temporary folder so they never touch the real one.
    pub content_folder: PathBuf,
    /// Where `rag-dataset` writes the training datasets it builds, one folder for each dataset. A
    /// relative path starts at the folder the command runs in. Tests set a temporary folder so
    /// they never touch the real one.
    pub dataset_folder: PathBuf,
    /// Where the answers of the models that write a dataset are kept, so that the same question is
    /// never paid for twice. A relative path starts at the folder the command runs in. Tests set
    /// a temporary folder so they never touch the real one.
    pub dataset_cache_folder: PathBuf,
    /// Not needed by every command, so a missing key is not an error here.
    pub gemini_api_key: Option<ApiKey>,
    /// The key of the Jev API, which the converter asks whether a page holds math. Only
    /// `rag-ingest pdf` uses it, and only when a page is left to convert, so a missing key is
    /// not an error here.
    pub jev_api_key: Option<ApiKey>,
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
    /// Reads the process environment, then the `.env` found in the current folder or its nearest
    /// parent, then defaults. The process environment is never changed.
    ///
    /// # Errors
    /// - [`ConfigError::CurrentFolder`] when the current folder cannot be found
    /// - [`ConfigError::DotenvRead`] and [`ConfigError::DotenvParse`] for an unreadable or invalid file
    pub fn load() -> Result<Config, ConfigError> {
        let current_folder = std::env::current_dir().map_err(ConfigError::CurrentFolder)?;
        let dotenv_file = current_folder
            .ancestors()
            .map(|folder| folder.join(DOTENV_FILE_NAME))
            .find(|candidate| candidate.is_file());
        Config::from_sources(process_environment, dotenv_file.as_deref())
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
        let or = |name: &str, default: &str| setting(name).unwrap_or_else(|| default.to_owned());
        Ok(Config {
            qdrant_url: or("QDRANT_URL", "http://localhost:6334"),
            qdrant_api_key: setting("QDRANT_API_KEY").map(ApiKey::new),
            falkordb_url: or("FALKORDB_URL", "falkor://localhost:6379"),
            falkordb_password: setting("FALKORDB_PASSWORD").map(ApiKey::new),
            falkordb_graph: or("FALKORDB_GRAPH", "quanty"),
            items_collection: or("QDRANT_ITEMS_COLLECTION", "items"),
            concepts_collection: or("QDRANT_CONCEPTS_COLLECTION", "concepts"),
            concept_cache_folder: or("CONCEPT_CACHE_DIR", "data/concept-cache").into(),
            concept_decision_log: or("CONCEPT_DECISION_LOG", "data/concept-decisions.jsonl").into(),
            content_folder: or("CONTENT_DIR", "content").into(),
            dataset_folder: or("DATASET_DIR", "datasets").into(),
            dataset_cache_folder: or("DATASET_CACHE_DIR", "data/dataset-cache").into(),
            gemini_api_key: setting("EMBEDDING_GEMINI_API_KEY").map(ApiKey::new),
            jev_api_key: setting("CONVERTER_JEV_API_KEY").map(ApiKey::new),
        })
    }
}

fn process_environment(name: &str) -> Option<String> {
    std::env::var(name).ok()
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
