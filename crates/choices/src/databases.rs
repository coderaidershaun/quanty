//! The databases a person has saved and the one chosen, kept in a file beside `.env` that every
//! program reads, so they all use the same library.

use crate::ApiKey;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The file of saved databases in the project home folder. It holds keys, so only its owner can
/// read it.
pub const DATABASES_FILE: &str = ".quanty-databases.json";
/// The name of the database that the environment, `.env` and the defaults describe. It is never
/// saved.
pub const LOCAL_DATABASE: &str = "Local";

/// Failures while reading, validating or saving the databases.
#[derive(Debug, thiserror::Error)]
pub enum DatabasesError {
    #[error("could not read the databases at {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    // The error of the parser is left out on purpose: it quotes the value it met, which may be a
    // key.
    #[error("the databases at {} are not valid at line {line}, column {column}", path.display())]
    Parse {
        path: PathBuf,
        line: usize,
        column: usize,
    },
    #[error("could not serialize the databases for {}", path.display())]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not save the databases at {}", path.display())]
    Save {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the chosen database {name:?} is not one of the saved ones")]
    ChosenNotSaved { name: String },
    #[error("the database {name:?} cannot be used: {what}")]
    Invalid { name: String, what: &'static str },
}

/// One place a library lives: its two stores and the folder of its converted pages.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Database {
    pub name: String,
    pub qdrant_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qdrant_api_key: Option<ApiKey>,
    pub falkordb_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub falkordb_password: Option<ApiKey>,
    pub falkordb_graph: String,
    pub content_folder: PathBuf,
}

impl Database {
    /// Whether this entry is the reserved one that the settings describe.
    pub fn is_local(&self) -> bool {
        is_local_name(&self.name)
    }

    /// Check an entry before it is saved. Local itself is never checked.
    ///
    /// # Errors
    /// [`DatabasesError::Invalid`] for a blank name, address or graph, the name Local in any
    /// case, a content folder that is blank or not absolute, and a key or password that is not
    /// printable ASCII, because Qdrant sends its key as a header and the client cannot send
    /// anything else.
    pub fn validate(&self) -> Result<(), DatabasesError> {
        let invalid = |what| DatabasesError::Invalid {
            name: self.name.clone(),
            what,
        };
        if is_blank(&self.name) {
            return Err(invalid("the name is blank"));
        }
        if self.is_local() {
            return Err(invalid("the name Local is taken by the settings"));
        }
        if is_blank(&self.qdrant_url) {
            return Err(invalid("the Qdrant address is blank"));
        }
        if is_blank(&self.falkordb_url) {
            return Err(invalid("the FalkorDB address is blank"));
        }
        if is_blank(&self.falkordb_graph) {
            return Err(invalid("the graph name is blank"));
        }
        if is_blank(&self.content_folder.to_string_lossy()) {
            return Err(invalid("the content folder is blank"));
        }
        if !self.content_folder.is_absolute() {
            return Err(invalid("the content folder is not an absolute path"));
        }
        if !self.qdrant_api_key.as_ref().is_none_or(is_printable) {
            return Err(invalid("the Qdrant API key is not printable ASCII"));
        }
        if !self.falkordb_password.as_ref().is_none_or(is_printable) {
            return Err(invalid("the FalkorDB password is not printable ASCII"));
        }
        Ok(())
    }
}

/// The saved databases and the name of the chosen one. Local is never saved.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Databases {
    pub chosen: Option<String>,
    pub saved: Vec<Database>,
}

/// What the file says is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chosen<'a> {
    Nothing,
    Local,
    Saved(&'a Database),
}

impl Databases {
    /// A file that chooses Local and saves nothing.
    pub fn choosing_local() -> Databases {
        Databases {
            chosen: Some(LOCAL_DATABASE.into()),
            saved: Vec::new(),
        }
    }

    /// Read the saved databases, or none chosen when the file is absent.
    ///
    /// # Errors
    /// - [`DatabasesError::Read`] when the file exists but cannot be read
    /// - [`DatabasesError::Parse`] when it is not a databases file, with the place of the fault
    pub fn load_from(path: &Path) -> Result<Databases, DatabasesError> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| DatabasesError::Parse {
                path: path.to_owned(),
                line: error.line(),
                column: error.column(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Databases::default()),
            Err(source) => Err(DatabasesError::Read {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// Atomically replace the file after checking every saved entry and the chosen name.
    ///
    /// # Errors
    /// - [`DatabasesError::Invalid`] and [`DatabasesError::ChosenNotSaved`] before anything is
    ///   written
    /// - [`DatabasesError::Serialize`] and [`DatabasesError::Save`] when writing fails
    pub fn save_to(&self, path: &Path) -> Result<(), DatabasesError> {
        use std::io::Write;
        self.saved.iter().try_for_each(Database::validate)?;
        self.chosen()?;
        let save_error = |source| DatabasesError::Save {
            path: path.to_owned(),
            source,
        };
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|source| DatabasesError::Serialize {
                path: path.to_owned(),
                source,
            })?;
        let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))
            .map_err(save_error)?;
        file.write_all(&bytes).map_err(save_error)?;
        file.persist(path)
            .map_err(|error| save_error(error.error))?;
        Ok(())
    }

    /// The database in use according to the file.
    ///
    /// # Errors
    /// [`DatabasesError::ChosenNotSaved`] when the chosen name is neither Local nor saved.
    pub fn chosen(&self) -> Result<Chosen<'_>, DatabasesError> {
        let Some(name) = &self.chosen else {
            return Ok(Chosen::Nothing);
        };
        if is_local_name(name) {
            return Ok(Chosen::Local);
        }
        self.saved
            .iter()
            .find(|database| database.name == *name)
            .map(Chosen::Saved)
            .ok_or_else(|| DatabasesError::ChosenNotSaved { name: name.clone() })
    }

    /// Save an entry, replacing the saved entry of the same name.
    pub fn put(&mut self, database: Database) {
        match self
            .saved
            .iter_mut()
            .find(|saved| saved.name == database.name)
        {
            Some(saved) => *saved = database,
            None => self.saved.push(database),
        }
    }

    /// Remove a saved entry. Nothing is chosen afterwards if it was the chosen one.
    pub fn forget(&mut self, name: &str) {
        self.saved.retain(|database| database.name != name);
        if self.chosen.as_deref() == Some(name) {
            self.chosen = None;
        }
    }
}

fn is_local_name(name: &str) -> bool {
    name.eq_ignore_ascii_case(LOCAL_DATABASE)
}

fn is_blank(text: &str) -> bool {
    text.trim().is_empty()
}

fn is_printable(key: &ApiKey) -> bool {
    key.expose()
        .bytes()
        .all(|byte| (b' '..=b'~').contains(&byte))
}
