//! How the program finds its home folder and its settings before the window opens.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::contract::StartupFacts;

const ENV_FILE: &str = ".env";
const HOME_VARIABLE: &str = "QUANTY_HOME";
/// The folders where a program started from Finder does not look: where `claude` is put by its
/// installer (under the user's home), and where Homebrew puts Poppler.
const HOME_TOOL_FOLDER: &str = ".local/bin";
const TOOL_FOLDERS: [&str; 2] = ["/opt/homebrew/bin", "/usr/local/bin"];

/// The folder that holds `.env`, `content/` and `data/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    pub folder: PathBuf,
    pub env_file: Option<PathBuf>,
}

/// Finder starts a program in `/`, so the search for a `.env` also starts from the program's own
/// folder.
///
/// # Errors
/// Whatever the operating system says when the current folder cannot be found.
pub fn find_home(flag: Option<&Path>) -> std::io::Result<Home> {
    let current = std::env::current_dir()?;
    let program = std::env::current_exe()
        .ok()
        .and_then(|program| program.parent().map(Path::to_path_buf));
    let variable = std::env::var_os(HOME_VARIABLE).filter(|value| !value.is_empty());
    Ok(home_from(
        flag,
        variable.as_deref(),
        &current,
        program.as_deref(),
    ))
}

fn home_from(
    flag: Option<&Path>,
    variable: Option<&OsStr>,
    current: &Path,
    program: Option<&Path>,
) -> Home {
    let given = flag
        .map(Path::to_path_buf)
        .or_else(|| variable.map(PathBuf::from));
    if let Some(folder) = given {
        // The program moves into this folder next, so a relative path must mean what it
        // means now.
        let folder = std::path::absolute(&folder).unwrap_or(folder);
        return home_at(folder);
    }
    let nearest = |start: &Path| {
        start
            .ancestors()
            .find(|folder| holds_env(folder))
            .map(Path::to_path_buf)
    };
    match nearest(current).or_else(|| program.and_then(nearest)) {
        Some(folder) => home_at(folder),
        None => Home {
            folder: current.to_path_buf(),
            env_file: None,
        },
    }
}

fn holds_env(folder: &Path) -> bool {
    folder.join(ENV_FILE).is_file()
}

fn home_at(folder: PathBuf) -> Home {
    let env_file = holds_env(&folder).then(|| folder.join(ENV_FILE));
    Home { folder, env_file }
}

/// The tool folders are appended, so the user's own `PATH` wins.
pub fn search_path() -> OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    path_with(&path, home.as_deref())
}

fn path_with(path: &OsStr, home: Option<&Path>) -> OsString {
    let mut folders: Vec<PathBuf> = std::env::split_paths(path).collect();
    let extra = home
        .map(|home| home.join(HOME_TOOL_FOLDER))
        .into_iter()
        .chain(TOOL_FOLDERS.iter().map(PathBuf::from));
    for folder in extra {
        if !folders.contains(&folder) && folder.is_dir() {
            folders.push(folder);
        }
    }
    std::env::join_paths(folders).unwrap_or_else(|_| path.to_owned())
}

impl Home {
    pub fn facts(&self, fixture: Option<&str>) -> StartupFacts {
        tracing::info!(
            home = %self.folder.display(),
            env_file = ?self.env_file,
            path = ?std::env::var_os("PATH"),
            "started"
        );
        StartupFacts {
            home: self.folder.clone(),
            env_file: self.env_file.clone(),
            fixture: fixture.map(str::to_owned),
            anthropic_api_key_set: std::env::var_os("ANTHROPIC_API_KEY")
                .is_some_and(|value| !value.is_empty()),
        }
    }
}
