//! Checks that the `claude` command line tool is signed in, without asking it anything that
//! costs money.

use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use tokio::process::Command;

const STATUS_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(thiserror::Error, Debug)]
pub enum ClaudeCliError {
    #[error("could not start `claude auth status`; is the claude CLI installed and on the PATH?")]
    Start(#[source] std::io::Error),

    #[error("`claude auth status` did not finish within {seconds} seconds")]
    TimedOut { seconds: u64 },

    #[error(
        "claude is not signed in: `claude auth status` ended with {status}, stderr: {stderr:?}"
    )]
    NotSignedIn { status: ExitStatus, stderr: String },
}

/// Ok when `claude auth status` exits with success. That command makes no model call. Its
/// standard output is thrown away, because it can hold the account name.
///
/// # Errors
/// - [`ClaudeCliError::Start`] when the command cannot be started
/// - [`ClaudeCliError::TimedOut`] when it does not finish in time
/// - [`ClaudeCliError::NotSignedIn`] when it exits with failure
pub async fn check_signed_in() -> Result<(), ClaudeCliError> {
    let mut command = Command::new("claude");
    command
        .args(["auth", "status"])
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(STATUS_TIMEOUT, command.output())
        .await
        .map_err(|_| ClaudeCliError::TimedOut {
            seconds: STATUS_TIMEOUT.as_secs(),
        })?
        .map_err(ClaudeCliError::Start)?;
    if output.status.success() {
        return Ok(());
    }
    Err(ClaudeCliError::NotSignedIn {
        status: output.status,
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}
