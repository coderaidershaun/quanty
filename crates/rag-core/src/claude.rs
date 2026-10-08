//! Everything that starts the `claude` command line tool: the check that it is signed in, which
//! costs nothing, and the answers to questions, which are billed to the subscription.

use std::ffi::{OsStr, OsString};
use std::process::{ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Map, Value};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::llm::{Llm, LlmError, Question};

const STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const QUESTION_TIMEOUT: Duration = Duration::from_secs(120);

const API_KEY_VARIABLE: &str = "ANTHROPIC_API_KEY";
const API_KEY_SUFFIX: &str = "_API_KEY";

// The variables a Claude Code session sets for its children. A child that inherits them thinks it
// is a nested session. They are removed by name, not by prefix: `CLAUDE_CONFIG_DIR` and
// `CLAUDE_CODE_OAUTH_TOKEN` are how some sign-ins work.
// SMELL: this list is a second copy of the one the converter keeps, and nothing keeps the two the
// same.
const SESSION_VARIABLES: [&str; 13] = [
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_BRIDGE_SESSION_ID",
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS",
];

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

/// `claude auth status` makes no model call. Its standard output is thrown away, because it can
/// hold the account name.
///
/// # Errors
/// - [`ClaudeCliError::Start`] when the command cannot be started
/// - [`ClaudeCliError::TimedOut`] when it does not finish in time
/// - [`ClaudeCliError::NotSignedIn`] when it exits with failure
pub async fn check_signed_in() -> Result<(), ClaudeCliError> {
    auth_status(Command::new("claude")).await
}

async fn auth_status(mut command: Command) -> Result<(), ClaudeCliError> {
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

/// Answers each question by starting one locked-down `claude -p` process. It runs on the
/// subscription that `claude` is signed in to, and it refuses to start while `ANTHROPIC_API_KEY`
/// is set, because `claude` would then bill the API.
// Do not derive `Debug`: this holds the whole environment, which can hold the keys of other
// services.
pub struct ClaudeCli {
    model: String,
    environment: Vec<(OsString, OsString)>,
}

impl ClaudeCli {
    pub fn new(model: impl Into<String>) -> ClaudeCli {
        ClaudeCli::with_environment(model, std::env::vars_os())
    }

    /// A caller that must control the `PATH` or the key has no need to change the environment of
    /// the whole process.
    pub fn with_environment<K, V>(
        model: impl Into<String>,
        environment: impl IntoIterator<Item = (K, V)>,
    ) -> ClaudeCli
    where
        K: Into<OsString>,
        V: Into<OsString>,
    {
        ClaudeCli {
            model: model.into(),
            environment: environment
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        }
    }

    fn api_key_is_set(&self) -> bool {
        self.environment
            .iter()
            .any(|(name, value)| name == API_KEY_VARIABLE && !value.is_empty())
    }

    /// `claude` with the variables it may get and no others, so the check of the sign-in and a
    /// question see the same sign-in.
    fn claude(&self) -> Command {
        let mut command = Command::new("claude");
        command.env_clear();
        for (name, value) in &self.environment {
            if !is_withheld(name) {
                command.env(name, value);
            }
        }
        command
    }

    fn command(&self, question: Question<'_>) -> Command {
        // Never add `--bare`: it skips the sign-in of the subscription, so `claude` would find no
        // login.
        let mut command = self.claude();
        command
            .arg("-p")
            // Do not remove: without it the run loads the instructions, hooks and plugins of the
            // folder it starts in and of the user, and a hook could fire for every question.
            .arg("--safe-mode")
            // Do not give tools: the flag is followed by one empty argument, which means no tool
            // at all, so text inside an item cannot make the model write, run or fetch anything.
            .args(["--tools", ""])
            .args(["--model", self.model.as_str()])
            .args(["--output-format", "json"])
            .args(["--json-schema", question.schema])
            .args(["--system-prompt", question.system_prompt])
            .arg("--no-session-persistence")
            // `claude` starts in the temporary folder, so that a call is the same wherever this
            // program is started.
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // Without this, `claude` lets the model think for thousands of words before it answers
        // an item: measured at ten times the cost and twenty times the time, for the same answer.
        command.env("MAX_THINKING_TOKENS", "0");
        command
    }
}

/// Variables that `claude` must not get: those of a running session, and every key of every
/// service, because `claude` runs on its own sign-in and needs no key.
fn is_withheld(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    SESSION_VARIABLES.contains(&name.as_ref()) || name.ends_with(API_KEY_SUFFIX)
}

impl Llm for ClaudeCli {
    fn model(&self) -> &str {
        &self.model
    }

    async fn check_ready(&self) -> Result<(), LlmError> {
        if self.api_key_is_set() {
            return Err(LlmError::ApiKeySet);
        }
        auth_status(self.claude())
            .await
            .map_err(|error| match error {
                ClaudeCliError::Start(source) => LlmError::Start(source),
                ClaudeCliError::TimedOut { seconds } => LlmError::TimedOut { seconds },
                ClaudeCliError::NotSignedIn { status, stderr } => LlmError::NotSignedIn {
                    message: format!(
                        "`claude auth status` ended with {status}, stderr: {stderr:?}"
                    ),
                },
            })
    }

    async fn ask(&self, question: Question<'_>) -> Result<Value, LlmError> {
        if self.api_key_is_set() {
            return Err(LlmError::ApiKeySet);
        }
        let started = Instant::now();
        // Dropping the future of a question that timed out kills the process.
        let output = tokio::time::timeout(
            QUESTION_TIMEOUT,
            run(self.command(question), question.input),
        )
        .await
        .map_err(|_| LlmError::TimedOut {
            seconds: QUESTION_TIMEOUT.as_secs(),
        })??;
        read_answer(&output, &self.model, started.elapsed())
    }
}

async fn run(mut command: Command, input: &str) -> Result<Output, LlmError> {
    let mut child = command.spawn().map_err(LlmError::Start)?;
    let mut stdin = child.stdin.take().expect("the standard input was piped");
    let write = async move {
        // `claude` can end before it reads its input, as it does when it is not signed in. That
        // is not the error: the reason is in its output, which is read next.
        if let Err(error) = stdin.write_all(input.as_bytes()).await {
            tracing::debug!(%error, "claude stopped reading its input");
        }
        drop(stdin);
    };
    let ((), output) = tokio::join!(write, child.wait_with_output());
    output.map_err(LlmError::Wait)
}

// Do not add `deny_unknown_fields`: `claude` adds new fields to this object over time.
#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    is_error: Option<bool>,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    errors: Option<Vec<String>>,
    #[serde(default)]
    api_error_status: Option<Value>,
    #[serde(default)]
    structured_output: Option<Value>,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default, rename = "modelUsage")]
    model_usage: Map<String, Value>,
}

/// Standard output is read before the exit code: a run that fails still prints a JSON result that
/// says why.
fn read_answer(output: &Output, asked_model: &str, elapsed: Duration) -> Result<Value, LlmError> {
    let mut envelope: Envelope =
        serde_json::from_slice(&output.stdout).map_err(|source| LlmError::UnreadableOutput {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            source,
        })?;
    log_cost(&envelope, asked_model, elapsed);
    match envelope.structured_output.take() {
        Some(value) if output.status.success() => Ok(value),
        _ => Err(failure(output, &envelope)),
    }
}

fn log_cost(envelope: &Envelope, asked_model: &str, elapsed: Duration) {
    let mut names = envelope.model_usage.keys();
    let model = match (names.next(), names.next()) {
        (Some(only), None) => only.as_str(),
        _ => asked_model,
    };
    tracing::info!(
        model,
        cost_usd = envelope.total_cost_usd.unwrap_or(0.0),
        seconds = elapsed.as_secs_f64(),
        "claude answered"
    );
}

/// Names the way a run failed. The words of the message are read only when the run is marked as
/// failed: with exit code 0 the message can be the model's own words, and a text about a rate
/// limit must not stop a whole run.
fn failure(output: &Output, envelope: &Envelope) -> LlmError {
    let message = reason_of(output, envelope);
    let marked_failed = !output.status.success() || envelope.is_error == Some(true);
    let lower = message.to_lowercase();
    let status = envelope.api_error_status.as_ref().and_then(Value::as_u64);
    if status == Some(429) || (marked_failed && says_usage_limit(&lower)) {
        LlmError::UsageLimit { message }
    } else if matches!(status, Some(401 | 403)) || (marked_failed && says_not_signed_in(&lower)) {
        LlmError::NotSignedIn { message }
    } else {
        LlmError::Failed {
            exit_code: output.status.code(),
            reason: message,
        }
    }
}

/// `claude` says why it failed in its result text, in its list of errors or on its standard
/// error. The first of the three that says anything is the reason.
fn reason_of(output: &Output, envelope: &Envelope) -> String {
    let errors = envelope.errors.as_ref().map(|errors| errors.join("; "));
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    [envelope.result.clone(), errors, Some(stderr)]
        .into_iter()
        .flatten()
        .map(|text| text.trim().to_owned())
        .find(|text| !text.is_empty())
        .unwrap_or_else(|| "no reason given".to_owned())
}

fn says_usage_limit(lower_case_message: &str) -> bool {
    lower_case_message.contains("usage limit")
        || lower_case_message.contains("rate limit")
        || (lower_case_message.contains("hit your") && lower_case_message.contains("limit"))
}

fn says_not_signed_in(lower_case_message: &str) -> bool {
    lower_case_message.contains("not logged in")
        || lower_case_message.contains("/login")
        || lower_case_message.contains("authentication")
}
