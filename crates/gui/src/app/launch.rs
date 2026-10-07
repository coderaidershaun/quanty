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

/// The home folder, found in this order: the one given, the one `QUANTY_HOME` names, the nearest
/// folder at or above the current one that holds a `.env`, the nearest at or above the
/// program's own folder that holds one (Finder starts a program in `/`), and last the current
/// folder with no `.env`.
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
        &|folder| folder.join(ENV_FILE).is_file(),
    ))
}

/// `find_home` with every lookup given: the flag, the variable, the two folders to start from,
/// and the question whether a folder holds a `.env`.
fn home_from(
    flag: Option<&Path>,
    variable: Option<&OsStr>,
    current: &Path,
    program: Option<&Path>,
    holds_env: &dyn Fn(&Path) -> bool,
) -> Home {
    let given = flag
        .map(Path::to_path_buf)
        .or_else(|| variable.map(PathBuf::from));
    if let Some(folder) = given {
        // The program moves into this folder next, so a relative path must mean what it
        // means now.
        let folder = std::path::absolute(&folder).unwrap_or(folder);
        return home_at(folder, holds_env);
    }
    let nearest = |start: &Path| {
        start
            .ancestors()
            .find(|folder| holds_env(folder))
            .map(Path::to_path_buf)
    };
    match nearest(current).or_else(|| program.and_then(nearest)) {
        Some(folder) => home_at(folder, holds_env),
        None => Home {
            folder: current.to_path_buf(),
            env_file: None,
        },
    }
}

fn home_at(folder: PathBuf, holds_env: &dyn Fn(&Path) -> bool) -> Home {
    let env_file = holds_env(&folder).then(|| folder.join(ENV_FILE));
    Home { folder, env_file }
}

/// The `PATH` the program runs with: the one it was given, then the tool folders that exist and
/// are not in it. Appended, so the user's own `PATH` wins.
pub fn search_path() -> OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    path_with(&path, home.as_deref(), &|folder| folder.is_dir())
}

fn path_with(path: &OsStr, home: Option<&Path>, exists: &dyn Fn(&Path) -> bool) -> OsString {
    let mut folders: Vec<PathBuf> = std::env::split_paths(path).collect();
    let extra = home
        .map(|home| home.join(HOME_TOOL_FOLDER))
        .into_iter()
        .chain(TOOL_FOLDERS.iter().map(PathBuf::from));
    for folder in extra {
        if !folders.contains(&folder) && exists(&folder) {
            folders.push(folder);
        }
    }
    std::env::join_paths(folders).unwrap_or_else(|_| path.to_owned())
}

impl Home {
    /// What the window needs to know about how it was started. It also logs where the program
    /// found its home, its settings and its tools.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A disk that has these `.env` files and these folders, and nothing else.
    fn disk(env_folders: &'static [&'static str]) -> impl Fn(&Path) -> bool {
        move |folder| env_folders.iter().any(|known| Path::new(known) == folder)
    }

    #[test]
    fn home_and_path_are_found_for_a_terminal_and_for_a_finder_launch() {
        let repository = disk(&["/work/quanty"]);

        // A terminal: the `.env` is above the folder the person is in.
        let terminal = home_from(
            None,
            None,
            Path::new("/work/quanty/crates/gui"),
            Some(Path::new("/work/quanty/target/debug")),
            &repository,
        );
        assert_eq!(terminal.folder, Path::new("/work/quanty"));
        assert_eq!(
            terminal.env_file.as_deref(),
            Some(Path::new("/work/quanty/.env"))
        );

        // Finder starts the program in `/`: the `.env` is above the program instead.
        let finder = home_from(
            None,
            None,
            Path::new("/"),
            Some(Path::new("/work/quanty/target/release")),
            &repository,
        );
        assert_eq!(finder, terminal);

        // The flag beats the variable, and the variable beats the search.
        let flagged = home_from(
            Some(Path::new("/flag")),
            Some(OsStr::new("/variable")),
            Path::new("/work/quanty"),
            None,
            &disk(&["/flag", "/variable", "/work/quanty"]),
        );
        assert_eq!(flagged.folder, Path::new("/flag"));
        let named = home_from(
            None,
            Some(OsStr::new("/variable")),
            Path::new("/work/quanty"),
            None,
            &disk(&["/work/quanty"]),
        );
        assert_eq!(
            (named.folder.as_path(), named.env_file),
            (Path::new("/variable"), None)
        );

        // No `.env` anywhere: the current folder, with no settings file.
        let bare = home_from(None, None, Path::new("/"), None, &disk(&[]));
        assert_eq!(
            bare,
            Home {
                folder: PathBuf::from("/"),
                env_file: None
            }
        );

        // Finder's own `PATH` has neither `claude` nor Poppler; both are added at the end.
        let exists = |folder: &Path| {
            ["/Users/me/.local/bin", "/opt/homebrew/bin"]
                .iter()
                .any(|known| Path::new(known) == folder)
        };
        let finder_path = path_with(
            OsStr::new("/usr/bin:/bin"),
            Some(Path::new("/Users/me")),
            &exists,
        );
        assert_eq!(
            finder_path,
            OsString::from("/usr/bin:/bin:/Users/me/.local/bin:/opt/homebrew/bin"),
            "a folder that is not there is left out, and the person's folders stay first"
        );
        let terminal_path = path_with(
            OsStr::new("/opt/homebrew/bin:/usr/bin"),
            Some(Path::new("/Users/me")),
            &exists,
        );
        assert_eq!(
            terminal_path,
            OsString::from("/opt/homebrew/bin:/usr/bin:/Users/me/.local/bin"),
            "a folder that is already there is not added again"
        );
    }
}
