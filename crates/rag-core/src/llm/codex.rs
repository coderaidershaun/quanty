//! Structured, headless Codex calls using the installed CLI's own login.
use crate::{Llm, LlmError, LlmReply, ModelUsage, Question, Usage, UsageTally};
use serde_json::Value;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::{io::AsyncWriteExt, process::Command};

/// Failures while preparing or running the installed Codex command.
#[derive(Debug, thiserror::Error)]
pub enum CodexError {
    #[error("could not create a temporary folder for codex")]
    TemporaryFolder(#[source] std::io::Error),
    #[error("invalid codex output schema")]
    Schema(#[source] serde_json::Error),
    #[error("could not write the codex output schema at {}", path.display())]
    WriteSchema {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not encode codex configuration")]
    Configuration(#[source] serde_json::Error),
    #[error("could not start codex; check that it is installed and on PATH")]
    Spawn(#[source] std::io::Error),
    #[error("could not send the prompt to codex")]
    WritePrompt(#[source] std::io::Error),
    #[error("could not wait for codex to finish")]
    Wait(#[source] std::io::Error),
    #[error("codex did not answer within {seconds} seconds")]
    TimedOut { seconds: u64 },
    #[error("codex returned invalid JSONL")]
    Response(#[source] serde_json::Error),
    #[error("codex failed: {reason}")]
    Failed { reason: String },
    #[error("codex ended without completing the turn")]
    IncompleteTurn,
    #[error("codex completed without an answer")]
    MissingAnswer,
    #[error("codex answer is not JSON")]
    Answer(#[source] serde_json::Error),
    #[error("codex login check timed out")]
    LoginTimedOut,
    #[error("codex is not signed in; run codex login: {reason}")]
    NotSignedIn { reason: String },
}

/// A client that uses the installed Codex command and its subscription login.
pub struct CodexCli {
    model: String,
    thinking: String,
    environment: Vec<(OsString, OsString)>,
}
impl CodexCli {
    /// Use the current process environment and the CLI’s existing login.
    pub fn new(model: impl Into<String>, thinking: impl Into<String>) -> Self {
        Self::with_environment(model, thinking, std::env::vars_os())
    }
    /// Use an explicit environment, so callers can isolate the CLI process.
    pub fn with_environment<K: Into<OsString>, V: Into<OsString>>(
        model: impl Into<String>,
        thinking: impl Into<String>,
        env: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        Self {
            model: model.into(),
            thinking: thinking.into(),
            environment: env.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
        }
    }
    fn command(&self) -> Command {
        let program = self
            .environment
            .iter()
            .find(|(k, _)| k == "CODEX_BIN")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "codex".into());
        let mut command = Command::new(program);
        command
            .args(["-c", "forced_login_method=\"chatgpt\""])
            .env_clear()
            .envs(
                self.environment
                    .iter()
                    .filter(|(k, _)| {
                        !k.to_string_lossy().ends_with("_API_KEY")
                            && !k.to_string_lossy().starts_with("CODEX_THREAD")
                            && k != "CODEX_SESSION_ID"
                    })
                    .map(|(k, v)| (k, v)),
            )
            .kill_on_drop(true);
        command
    }
    /// Attach page images to one structured question.
    ///
    /// # Errors
    /// Returns [`LlmError::Codex`] when preparation, the command or its reply fails.
    pub async fn ask_with_images(
        &self,
        question: Question<'_>,
        images: &[PathBuf],
        timeout: Duration,
    ) -> Result<LlmReply, LlmError> {
        tokio::time::timeout(timeout, self.run(question, images))
            .await
            .map_err(|_| CodexError::TimedOut {
                seconds: timeout.as_secs(),
            })?
    }

    async fn run(&self, question: Question<'_>, images: &[PathBuf]) -> Result<LlmReply, LlmError> {
        let dir = tempfile::tempdir().map_err(CodexError::TemporaryFolder)?;
        let schema = dir.path().join("schema.json");
        let mut value: Value = serde_json::from_str(question.schema).map_err(CodexError::Schema)?;
        strict_schema(&mut value);
        std::fs::write(&schema, value.to_string()).map_err(|source| CodexError::WriteSchema {
            path: schema.clone(),
            source,
        })?;
        let mut command = self.command();
        command.args([
            "exec",
            "--ignore-user-config",
            "--ignore-rules",
            "--skip-git-repo-check",
            "--ephemeral",
            "--json",
            "--sandbox",
            "read-only",
            "--model",
            &self.model,
        ]);
        for setting in [
            "approval_policy=\"never\"",
            "project_doc_max_bytes=0",
            "web_search=\"disabled\"",
            "features.shell_tool=false",
            "features.plugins=false",
            "features.apps=false",
            "features.hooks=false",
            "features.memories=false",
            "features.multi_agent=false",
            "features.browser_use=false",
            "features.computer_use=false",
            "features.image_generation=false",
            "features.view_image=false",
            "features.skill_search=false",
        ] {
            command.args(["-c", setting]);
        }
        command.args([
            "-c",
            &format!(
                "developer_instructions={}",
                serde_json::to_string(question.system_prompt).map_err(CodexError::Configuration)?
            ),
            "-c",
            &format!(
                "model_reasoning_effort={}",
                serde_json::to_string(&self.thinking).map_err(CodexError::Configuration)?
            ),
        ]);
        command.arg("--output-schema").arg(&schema);
        for image in images {
            command.arg("--image").arg(image);
        }
        command
            .arg("-")
            .current_dir(dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(CodexError::Spawn)?;
        let mut stdin = child
            .stdin
            .take()
            .expect("the command was configured with piped stdin");
        let write = async {
            let result = stdin.write_all(question.input.as_bytes()).await;
            drop(stdin);
            result
        };
        let (written, output) = tokio::join!(write, child.wait_with_output());
        let output = output.map_err(CodexError::Wait)?;
        if let Err(e) = written
            && e.kind() != std::io::ErrorKind::BrokenPipe
        {
            return Err(CodexError::WritePrompt(e).into());
        }
        parse(
            &output.stdout,
            output.status.success(),
            &String::from_utf8_lossy(&output.stderr),
            &self.model,
        )
    }
}
impl Llm for CodexCli {
    fn cache_identity(&self) -> String {
        format!("codex:{}:{}", self.model, self.thinking)
    }
    fn model(&self) -> &str {
        &self.model
    }
    async fn check_ready(&self) -> Result<(), LlmError> {
        let output = tokio::time::timeout(
            Duration::from_secs(10),
            self.command()
                .args(["login", "status"])
                .stdin(Stdio::null())
                .output(),
        )
        .await
        .map_err(|_| CodexError::LoginTimedOut)?
        .map_err(CodexError::Spawn)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(CodexError::NotSignedIn {
                reason: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            }
            .into())
        }
    }
    async fn ask(&self, question: Question<'_>) -> Result<LlmReply, LlmError> {
        self.ask_with_images(question, &[], Duration::from_secs(300))
            .await
    }
}

fn parse(stdout: &[u8], success: bool, stderr: &str, model: &str) -> Result<LlmReply, LlmError> {
    let mut answer = None;
    let mut usage = None;
    let mut failure = None;
    for line in stdout.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        let event: Value = serde_json::from_slice(line).map_err(CodexError::Response)?;
        match event["type"].as_str() {
            Some("item.completed") if event["item"]["type"] == "agent_message" => {
                answer = event["item"]["text"].as_str().map(str::to_owned)
            }
            Some("turn.completed") => usage = Some(event["usage"].clone()),
            Some("turn.failed") => {
                failure = Some(
                    event["error"]["message"]
                        .as_str()
                        .unwrap_or("Codex turn failed")
                        .to_owned(),
                )
            }
            _ => {}
        }
    }
    if !success || failure.is_some() {
        return Err(CodexError::Failed {
            reason: failure.unwrap_or_else(|| stderr.trim().to_owned()),
        }
        .into());
    }
    let usage = usage.ok_or(CodexError::IncompleteTurn)?;
    let answer = answer.ok_or(CodexError::MissingAnswer)?;
    let value = serde_json::from_str(&answer).map_err(CodexError::Answer)?;
    let cached = usage["cached_input_tokens"].as_u64().unwrap_or(0);
    let tokens = Usage {
        input_tokens: usage["input_tokens"]
            .as_u64()
            .unwrap_or(0)
            .saturating_sub(cached),
        cache_read_tokens: cached,
        output_tokens: usage["output_tokens"].as_u64().unwrap_or(0),
        cache_write_tokens: 0,
    };
    Ok(LlmReply {
        value,
        usage: UsageTally::of(
            model,
            ModelUsage {
                tokens,
                reported_usd: None,
                estimated: false,
            },
        ),
    })
}

/// OpenAI structured output requires closed objects and every property listed as required.
fn strict_schema(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        if object.get("type").and_then(Value::as_str) == Some("object")
            || object.contains_key("properties")
        {
            object.insert("additionalProperties".into(), Value::Bool(false));
            if let Some(properties) = object.get("properties").and_then(Value::as_object) {
                object.insert(
                    "required".into(),
                    Value::Array(
                        properties
                            .keys()
                            .map(|k| Value::String(k.clone()))
                            .collect(),
                    ),
                );
            }
        }
        for key in ["properties", "$defs", "definitions", "patternProperties"] {
            if let Some(children) = object.get_mut(key).and_then(Value::as_object_mut) {
                for child in children.values_mut() {
                    strict_schema(child);
                }
            }
        }
        for key in ["items", "anyOf", "oneOf", "allOf", "not"] {
            if let Some(child) = object.get_mut(key) {
                strict_schema(child);
            }
        }
    } else if let Some(array) = value.as_array_mut() {
        for child in array {
            strict_schema(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    #[ignore = "uses the locally signed-in Codex subscription"]
    async fn live_codex_structured_reply() {
        let cli = CodexCli::new("gpt-6.1-sol", "low");
        cli.check_ready().await.unwrap();
        let reply = cli.ask(Question { system_prompt:"Reply with the requested JSON.", schema:r#"{"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}"#, input:"Set ok to true." }).await.unwrap();
        assert_eq!(reply.value, json!({"ok":true}));
        assert!(reply.usage.tokens().total() > 0);
    }
}
