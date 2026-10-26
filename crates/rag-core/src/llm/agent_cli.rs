//! Select the installed headless agent from the saved choice of one kind of work.
use crate::{ClaudeCli, CodexCli, Llm, LlmError, LlmReply, ModelSettings, Question, WorkKind};

/// The agent of the saved choice of one kind of work, or a settings error reported before any
/// question is asked.
pub enum AgentCli {
    Claude(ClaudeCli),
    Codex(CodexCli),
    Invalid(std::sync::Arc<crate::SettingsError>),
}
impl AgentCli {
    /// Use the saved choice of this kind of work, retaining any settings failure for readiness and
    /// question calls.
    pub fn for_work(work: WorkKind) -> Self {
        match ModelSettings::load_for(work) {
            Ok(settings) if settings.provider == "codex" => {
                Self::Codex(CodexCli::new(settings.model, settings.thinking))
            }
            Ok(settings) => {
                Self::Claude(ClaudeCli::new(settings.model).with_effort(settings.thinking))
            }
            Err(error) => Self::Invalid(std::sync::Arc::new(error)),
        }
    }
}
impl Llm for AgentCli {
    fn cache_identity(&self) -> String {
        match self {
            Self::Claude(client) => client.cache_identity(),
            Self::Codex(client) => client.cache_identity(),
            Self::Invalid(_) => "invalid".into(),
        }
    }
    fn model(&self) -> &str {
        match self {
            Self::Claude(client) => client.model(),
            Self::Codex(client) => client.model(),
            Self::Invalid(_) => "invalid",
        }
    }
    async fn check_ready(&self) -> Result<(), LlmError> {
        match self {
            Self::Claude(client) => client.check_ready().await,
            Self::Codex(client) => client.check_ready().await,
            Self::Invalid(error) => Err(LlmError::Configuration(error.clone())),
        }
    }
    async fn ask(&self, question: Question<'_>) -> Result<LlmReply, LlmError> {
        match self {
            Self::Claude(client) => client.ask(question).await,
            Self::Codex(client) => client.ask(question).await,
            Self::Invalid(error) => Err(LlmError::Configuration(error.clone())),
        }
    }
    async fn ask_long(&self, question: Question<'_>) -> Result<LlmReply, LlmError> {
        match self {
            Self::Claude(client) => client.ask_long(question).await,
            Self::Codex(client) => client.ask_long(question).await,
            Self::Invalid(error) => Err(LlmError::Configuration(error.clone())),
        }
    }
}
