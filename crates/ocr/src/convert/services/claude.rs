//! The one place the `claude` command is started to answer a single question, so every model call
//! is locked down the same way and every caller gets the same errors and usage figures.

use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use tokio::process::Command;

use super::jev::JEV_API_KEY_VARIABLE;

const API_KEY_VARIABLE: &str = "ANTHROPIC_API_KEY";

// The variables a Claude Code session sets for its children. A child that inherits them thinks
// it is a nested session. They are removed by name, not by prefix: `CLAUDE_CONFIG_DIR` and
// `CLAUDE_CODE_OAUTH_TOKEN` are how some logins work.
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

pub(super) struct ClaudeCall<'a> {
    pub model: &'a str,
    pub effort: Option<&'a str>,
    pub system_prompt: &'a str,
    /// The first words of the user prompt, such as "Categorise the PDF page".
    pub instruction: &'a str,
    /// Why the last reply was rejected, when this is a second try. It has no closing full stop.
    pub correction: Option<&'a str>,
    pub schema: &'a Value,
    pub file: &'a Path,
    /// A second file of the same page, such as a sharper picture. It must sit in the same
    /// folder as `file`, because the model can only read that folder.
    pub also_read: Option<&'a Path>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Answer<T> {
    pub value: T,
    pub usage: CallUsage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallUsage {
    /// The model that answered when `claude` names exactly one, otherwise the one asked for.
    pub model: String,
    pub cost_usd: f64,
    pub output_tokens: u64,
    pub thinking_tokens: u64,
    pub seconds: f64,
}

#[derive(thiserror::Error, Debug)]
pub enum ClaudeError {
    #[error(
        "ANTHROPIC_API_KEY is set, so claude could bill the API instead of the subscription; unset it and run again"
    )]
    ApiKeySet,

    #[error("page file is missing or unreadable at {}", path.display())]
    FileUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "claude can only read one folder, but {} and {} are in different folders",
        file.display(),
        also_read.display()
    )]
    FilesApart { file: PathBuf, also_read: PathBuf },

    #[error("could not start the claude command; check that it is installed and on PATH")]
    Spawn(#[source] std::io::Error),

    #[error("claude did not answer within {seconds} seconds")]
    TimedOut { seconds: u64 },

    #[error("claude failed ({status}) with no readable response, stderr {stderr:?}")]
    Exited {
        status: ExitStatus,
        stderr: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("claude response was not the expected json")]
    UnreadableResponse(#[source] serde_json::Error),

    #[error("claude's reply passed its schema but does not fit the expected type")]
    UnexpectedReply(#[source] serde_json::Error),

    #[error(
        "claude run ended with subtype {subtype}{} and no reply: {}",
        status_note(*.api_error_status),
        join_reasons(.result.as_deref(), .errors)
    )]
    RunFailed {
        subtype: String,
        result: Option<String>,
        errors: Vec<String>,
        api_error_status: Option<u16>,
    },
}

impl ClaudeError {
    /// True only for a failure that is the server's or the clock's fault. Everything else,
    /// including every way a usage limit might arrive, is final: retrying into a limit would
    /// only spend more of it.
    pub fn is_worth_retrying(&self) -> bool {
        match self {
            Self::TimedOut { .. } => true,
            Self::RunFailed {
                api_error_status: Some(status),
                ..
            } => *status >= 500,
            _ => false,
        }
    }
}

fn status_note(status: Option<u16>) -> String {
    status.map_or_else(String::new, |status| format!(" (status {status})"))
}

// SMELL: an empty `result` string counts as a reason, so the message ends with a bare colon.
fn join_reasons(result: Option<&str>, errors: &[String]) -> String {
    let reasons: Vec<&str> = result
        .into_iter()
        .chain(errors.iter().map(String::as_str))
        .collect();
    if reasons.is_empty() {
        "no reason given".to_owned()
    } else {
        reasons.join("; ")
    }
}

// Do not add `deny_unknown_fields`: `claude` adds new fields to this object over time.
#[derive(Deserialize)]
struct Envelope {
    subtype: String,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    api_error_status: Option<Value>,
    #[serde(default)]
    structured_output: Option<Value>,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    usage: UsageFigures,
    #[serde(default, rename = "modelUsage")]
    model_usage: Map<String, Value>,
}

#[derive(Deserialize, Default)]
struct UsageFigures {
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    output_tokens_details: OutputTokenDetails,
}

#[derive(Deserialize, Default)]
struct OutputTokenDetails {
    #[serde(default)]
    thinking_tokens: u64,
}

/// True when `ANTHROPIC_API_KEY` is set to something. `claude` would then bill the API instead
/// of the subscription, so nothing may be started.
pub(crate) fn api_key_is_set() -> bool {
    std::env::var_os(API_KEY_VARIABLE).is_some_and(|value| !value.is_empty())
}

/// The model gets one tool, `Read`, confined to the folder the page file sits in.
pub(super) async fn run<T: DeserializeOwned>(
    call: &ClaudeCall<'_>,
) -> Result<Answer<T>, ClaudeError> {
    if api_key_is_set() {
        return Err(ClaudeError::ApiKeySet);
    }
    let file_unreadable = |source| ClaudeError::FileUnreadable {
        path: call.file.to_path_buf(),
        source,
    };
    let page_file = std::fs::canonicalize(call.file).map_err(file_unreadable)?;
    // `claude` compares canonical paths, so the working folder must come from the canonical file.
    let page_folder = page_file.parent().ok_or_else(|| {
        file_unreadable(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the file has no parent folder",
        ))
    })?;
    let prompt = user_prompt(call, &page_file, page_folder)?;
    let mut command = locked_down_command(call, prompt, page_folder);

    let started = Instant::now();
    // Dropping the timed-out future drops the process handle, which kills `claude`.
    let output = tokio::time::timeout(call.timeout, command.output())
        .await
        .map_err(|_| ClaudeError::TimedOut {
            seconds: call.timeout.as_secs(),
        })?
        // SMELL: a failure while waiting for `claude` is also reported as a failure to start it.
        .map_err(ClaudeError::Spawn)?;

    read_answer(&output, call.model, started.elapsed())
}

fn user_prompt(
    call: &ClaudeCall<'_>,
    page_file: &Path,
    page_folder: &Path,
) -> Result<String, ClaudeError> {
    let mut prompt = match call.also_read {
        None => format!(
            "{} at {}. Read the whole file with the Read tool first, with no page range.",
            call.instruction,
            page_file.display()
        ),
        Some(also_read) => {
            let picture =
                std::fs::canonicalize(also_read).map_err(|source| ClaudeError::FileUnreadable {
                    path: also_read.to_path_buf(),
                    source,
                })?;
            if picture.parent() != Some(page_folder) {
                return Err(ClaudeError::FilesApart {
                    file: call.file.to_path_buf(),
                    also_read: also_read.to_path_buf(),
                });
            }
            format!(
                "{} at {}, and a sharper picture of the same page at {}. Read both files with the Read tool first, with no page range.",
                call.instruction,
                page_file.display(),
                picture.display()
            )
        }
    };
    if let Some(correction) = call.correction {
        prompt.push_str(&format!(
            " Your previous reply was rejected: {correction}. Do the page again and correct this."
        ));
    }
    Ok(prompt)
}

fn locked_down_command(call: &ClaudeCall<'_>, prompt: String, page_folder: &Path) -> Command {
    let mut command = Command::new("claude");
    // The prompt must come right after `-p`: `--tools` and `--allowedTools` take any number of
    // values and would swallow a prompt placed after them.
    command
        .arg("-p")
        .arg(prompt)
        .args(["--model", call.model])
        .args(["--output-format", "json"])
        .arg("--json-schema")
        .arg(call.schema.to_string())
        .args(["--system-prompt", call.system_prompt])
        // Do not remove: without this flag the run loads the instructions, hooks and plugins of
        // whatever project it is started in, and a hook could then fire for every page.
        .arg("--safe-mode")
        // Do not widen: Read is the only tool the model is given and anything else is refused, so
        // text on a page cannot make it write files, run commands or fetch anything.
        .args(["--tools", "Read"])
        .args(["--allowedTools", "Read"])
        .args(["--permission-mode", "dontAsk"])
        .arg("--no-session-persistence")
        // Do not remove: together with the working folder below, this stops text on a page from
        // making the model read any file outside the page's own folder.
        .arg("--restricted")
        .current_dir(page_folder)
        // Without this, `claude` waits a few seconds for input on the caller's stdin.
        .stdin(Stdio::null())
        .kill_on_drop(true);
    if let Some(effort) = call.effort {
        command.args(["--effort", effort]);
    }
    for variable in SESSION_VARIABLES {
        command.env_remove(variable);
    }
    // `claude` has no use for the Jev API key, so it never gets it.
    command.env_remove(JEV_API_KEY_VARIABLE);
    command
}

fn read_answer<T: DeserializeOwned>(
    output: &Output,
    asked_model: &str,
    elapsed: Duration,
) -> Result<Answer<T>, ClaudeError> {
    // Stdout is read before the exit status: a run that fails still prints a JSON result saying
    // why, and exits with a failure status.
    let envelope = match serde_json::from_slice::<Envelope>(&output.stdout) {
        Ok(envelope) => envelope,
        Err(source) if output.status.success() => {
            return Err(ClaudeError::UnreadableResponse(source));
        }
        Err(source) => {
            return Err(ClaudeError::Exited {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                source,
            });
        }
    };

    let reply = match envelope.structured_output {
        Some(reply) if envelope.subtype == "success" => reply,
        _ => {
            return Err(ClaudeError::RunFailed {
                subtype: envelope.subtype,
                result: envelope.result,
                errors: envelope.errors,
                api_error_status: envelope
                    .api_error_status
                    .and_then(|status| status.as_u64())
                    .and_then(|status| u16::try_from(status).ok()),
            });
        }
    };
    let value = serde_json::from_value(reply).map_err(ClaudeError::UnexpectedReply)?;

    let mut model_names = envelope.model_usage.keys();
    let model = match (model_names.next(), model_names.next()) {
        (Some(only), None) => only.clone(),
        _ => asked_model.to_owned(),
    };
    Ok(Answer {
        value,
        usage: CallUsage {
            model,
            cost_usd: envelope.total_cost_usd.unwrap_or(0.0),
            output_tokens: envelope.usage.output_tokens,
            thinking_tokens: envelope.usage.output_tokens_details.thinking_tokens,
            seconds: elapsed.as_secs_f64(),
        },
    })
}
