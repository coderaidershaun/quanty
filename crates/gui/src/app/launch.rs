//! How the program finds its home folder and its settings before the window opens.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::contract::StartupFacts;

/// The folder that holds `.env`, `content/` and `data/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    pub folder: PathBuf,
    pub env_file: Option<PathBuf>,
}

/// The home folder: the one given, or the current folder, with the `.env` that is in it.
///
/// # Errors
/// Whatever the operating system says when the current folder cannot be found.
pub fn find_home(flag: Option<&Path>) -> std::io::Result<Home> {
    let folder = match flag {
        Some(folder) => folder.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let env_file = Some(folder.join(".env")).filter(|file| file.is_file());
    Ok(Home { folder, env_file })
}

/// The `PATH` the program runs with.
pub fn search_path() -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

impl Home {
    /// What the window needs to know about how it was started.
    pub fn facts(&self, fixture: Option<&str>) -> StartupFacts {
        StartupFacts {
            home: self.folder.clone(),
            env_file: self.env_file.clone(),
            fixture: fixture.map(str::to_owned),
            anthropic_api_key_set: std::env::var_os("ANTHROPIC_API_KEY")
                .is_some_and(|value| !value.is_empty()),
        }
    }
}
