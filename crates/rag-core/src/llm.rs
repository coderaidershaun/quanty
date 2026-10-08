//! What a program may ask of a language model: one question in, one answer in JSON out. The
//! question fits any model, but the failures are those of the `claude` command.

use serde_json::Value;

/// One question to a language model. The answer must fit the schema.
#[derive(Debug, Clone, Copy)]
pub struct Question<'a> {
    pub system_prompt: &'a str,
    /// A JSON Schema, as JSON text.
    pub schema: &'a str,
    /// The whole message the model reads.
    pub input: &'a str,
}

pub trait Llm {
    /// The name of the model that answers. A caller that keeps answers uses it to tell the
    /// answers of two models apart.
    fn model(&self) -> &str;

    /// Says whether a question could be asked now. It asks none, so it costs nothing: a caller
    /// asks this before it pays another service for work that only the model can finish. A usage
    /// limit is not seen here, because only a question finds it.
    ///
    /// # Errors
    /// [`LlmError::ApiKeySet`], [`LlmError::Start`], [`LlmError::NotSignedIn`] and
    /// [`LlmError::TimedOut`] when no question can be answered until the person acts.
    fn check_ready(&self) -> impl Future<Output = Result<(), LlmError>> + Send;

    /// # Errors
    /// - [`LlmError::ApiKeySet`], [`LlmError::Start`], [`LlmError::NotSignedIn`] and
    ///   [`LlmError::UsageLimit`] when no question can be answered until the person acts
    /// - [`LlmError::TimedOut`], [`LlmError::Wait`], [`LlmError::UnreadableOutput`] and
    ///   [`LlmError::Failed`] when this question failed and another may not
    fn ask(&self, question: Question<'_>) -> impl Future<Output = Result<Value, LlmError>> + Send;
}

#[derive(thiserror::Error, Debug)]
pub enum LlmError {
    #[error(
        "ANTHROPIC_API_KEY is set, so claude could bill the API instead of the subscription; unset it and run again"
    )]
    ApiKeySet,

    #[error("could not start the claude command; check that it is installed and on the PATH")]
    Start(#[source] std::io::Error),

    #[error("claude is not signed in ({message}); sign in with the claude command and run again")]
    NotSignedIn { message: String },

    #[error("claude has reached its usage limit ({message}); run again when the limit resets")]
    UsageLimit { message: String },

    #[error("claude did not answer within {seconds} seconds")]
    TimedOut { seconds: u64 },

    #[error("claude was started but waiting for it to finish failed")]
    Wait(#[source] std::io::Error),

    #[error(
        "claude printed something that is not its JSON result (exit code {}, stderr {stderr:?})",
        exit_code_text(*exit_code)
    )]
    UnreadableOutput {
        exit_code: Option<i32>,
        stderr: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("claude failed (exit code {}): {reason}", exit_code_text(*exit_code))]
    Failed {
        exit_code: Option<i32>,
        reason: String,
    },
}

impl LlmError {
    /// True when the next question would fail the same way, so asking again only wastes time or
    /// usage.
    pub fn stops_the_run(&self) -> bool {
        matches!(
            self,
            LlmError::ApiKeySet
                | LlmError::Start(_)
                | LlmError::NotSignedIn { .. }
                | LlmError::UsageLimit { .. }
        )
    }
}

fn exit_code_text(exit_code: Option<i32>) -> String {
    match exit_code {
        Some(code) => code.to_string(),
        None => "none, the program was stopped by a signal".to_owned(),
    }
}
