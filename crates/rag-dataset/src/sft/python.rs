//! Runs one program that a model wrote, to see whether it prints the value that its answer states.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

/// How long one program may run before it is stopped.
const TIME_LIMIT: Duration = Duration::from_secs(10);
const PYTHON: &str = "python3";
const PROGRAM_FILE: &str = "program.py";

/// A program could not be put in a file to run, so it was not run.
#[derive(thiserror::Error, Debug)]
#[error(
    "could not write {} to run the code a model wrote; check that the temporary folder can be written to, or build without running the code (--no-run-code, or run_code: false)",
    path.display()
)]
pub struct ProgramError {
    pub path: PathBuf,
    #[source]
    pub source: std::io::Error,
}

/// True when `python3` starts and runs a program, so that a build can find out before it runs any.
pub(super) async fn is_usable() -> bool {
    usable(PYTHON, TIME_LIMIT).await
}

/// True when the program runs to the end within the time limit and, when the answer states a
/// value, prints it. A program that cannot run, fails or runs too long does not pass.
///
/// # Errors
/// [`ProgramError`] when the program cannot be written to a temporary folder.
pub(super) async fn passes(program: &str, final_value: Option<&str>) -> Result<bool, ProgramError> {
    let Some(output) = printed(PYTHON, program, TIME_LIMIT).await? else {
        return Ok(false);
    };
    Ok(final_value.is_none_or(|value| output.contains(value)))
}

async fn usable(python: &str, limit: Duration) -> bool {
    let run = command(python).args(["-I", "-c", "pass"]).output();
    matches!(timeout(limit, run).await, Ok(Ok(output)) if output.status.success())
}

// SMELL: the program is not sandboxed. It runs with the rights of the person who builds, so it can
// read their files and use the network. Only its environment, its folder and its time are limited.
// SMELL: the whole of what the program prints is read into memory, so a program that prints
// without end fills memory until the time limit stops it.
/// What the program printed, or `None` when it could not be started, failed, or ran past the
/// limit. It runs alone in a folder of its own that is removed afterwards.
async fn printed(
    python: &str,
    program: &str,
    limit: Duration,
) -> Result<Option<String>, ProgramError> {
    let folder = tempfile::tempdir().map_err(|source| ProgramError {
        path: std::env::temp_dir(),
        source,
    })?;
    let file = folder.path().join(PROGRAM_FILE);
    std::fs::write(&file, program).map_err(|source| ProgramError {
        path: file.clone(),
        source,
    })?;
    let run = command(python)
        .arg("-I")
        .arg(&file)
        .current_dir(folder.path())
        .output();
    match timeout(limit, run).await {
        Ok(Ok(output)) if output.status.success() => {
            Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
        }
        _ => Ok(None),
    }
}

/// The environment is cleared so that a program a model wrote cannot read the keys and tokens of
/// the person who builds. Only `PATH` stays: without it `python3` is looked for in the system
/// folders, and not where the person installed it. A program that runs past the limit is dropped,
/// and dropping it kills it.
fn command(python: &str) -> Command {
    let mut command = Command::new(python);
    command
        .env_clear()
        .envs(std::env::var_os("PATH").map(|path| ("PATH", path)))
        .stdin(Stdio::null())
        .kill_on_drop(true);
    command
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[tokio::test]
    async fn a_program_that_runs_too_long_is_stopped() {
        let limit = Duration::from_millis(300);
        let started = Instant::now();

        let printed = printed(PYTHON, "import time\ntime.sleep(30)", limit)
            .await
            .unwrap();

        assert_eq!(printed, None);
        let waited = started.elapsed();
        assert!(
            waited >= limit && waited < Duration::from_secs(5),
            "the program should have run up to the limit and then been stopped: {waited:?}"
        );
    }

    #[tokio::test]
    async fn a_python_that_cannot_be_started_is_not_usable() {
        assert!(!usable("python3-that-does-not-exist", TIME_LIMIT).await);
    }
}
