//! The six things the app needs before it can work, what is known about each, which failures
//! name one of them, and what the last read of each paid account found.

use super::failure::{Failure, FailureKind};

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Service {
    Qdrant,
    FalkorDb,
    Claude,
    EmbeddingKey,
    ConverterKey,
    Poppler,
}

impl Service {
    pub const ALL: [Service; 6] = [
        Service::Qdrant,
        Service::FalkorDb,
        Service::Claude,
        Service::EmbeddingKey,
        Service::ConverterKey,
        Service::Poppler,
    ];
}

#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
)]
pub enum ServiceState {
    #[default]
    Unknown,
    Checking,
    Up {
        detail: String,
    },
    Down(Failure),
}

#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
)]
pub struct StartupFacts {
    /// The name of the scene when the app runs from fixtures.
    pub fixture: Option<String>,
    pub anthropic_api_key_set: bool,
    pub model_choices: choices::WorkChoices,
    pub running: Vec<RunningJob>,
}

impl Failure {
    /// A slow or failed call names no service: the service may be fine.
    pub fn service(&self) -> Option<Service> {
        match self.kind {
            FailureKind::QdrantDown => Some(Service::Qdrant),
            FailureKind::FalkorDbDown => Some(Service::FalkorDb),
            FailureKind::EmbeddingKeyMissing | FailureKind::EmbeddingOutOfCredit => {
                Some(Service::EmbeddingKey)
            }
            FailureKind::ConverterKeyMissing | FailureKind::ConverterOutOfCredit => {
                Some(Service::ConverterKey)
            }
            FailureKind::AgentFailed
            | FailureKind::ClaudeMissing
            | FailureKind::ClaudeSignedOut
            | FailureKind::ClaudeUsageLimit
            | FailureKind::ClaudeApiKeySet => Some(Service::Claude),
            FailureKind::PopplerMissing => Some(Service::Poppler),
            FailureKind::EmbeddingFailed
            | FailureKind::TimedOut
            | FailureKind::BadFile
            | FailureKind::PageFailed
            | FailureKind::ChapterTaken
            | FailureKind::MediaExists
            | FailureKind::SourceMissing
            | FailureKind::WriteFailed
            | FailureKind::Internal => None,
        }
    }
}

/// The two paid services whose accounts the footer shows.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Provider {
    Google,
    Jev,
}

impl Provider {
    pub const ALL: [Provider; 2] = [Provider::Google, Provider::Jev];

    /// The page where a person sees and tops up the credit: the AI Studio billing page, and the
    /// Typesafe console, which documents no billing page of its own.
    pub fn billing_page(self) -> &'static str {
        match self {
            Provider::Google => "https://aistudio.google.com/billing",
            Provider::Jev => "https://console.typesafe.ai/",
        }
    }
}

/// What a read found of one provider's account. It never holds an amount: neither provider gives
/// one for its key. No variant holds a string from a provider, so nothing it says, and no key,
/// can reach the screen.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum Balance {
    KeyMissing,
    KeyRefused,
    OutOfCredit,
    /// The key was taken, and the provider gives no way to read a balance with it.
    NotOffered,
    Unread(ReadFailure),
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum ReadFailure {
    Unreachable,
    TimedOut,
    /// Any other HTTP status.
    Status(u16),
    /// The read itself failed inside quanty, for example its task panicked.
    Internal,
}

/// What one read found, one entry for each provider.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct BalanceReads {
    pub google: Balance,
    pub jev: Balance,
}

impl BalanceReads {
    pub fn of(&self, provider: Provider) -> Balance {
        match provider {
            Provider::Google => self.google,
            Provider::Jev => self.jev,
        }
    }
}

/// Paid work that continues on the server while a tab is closed.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct RunningJob {
    pub kind: JobKind,
    pub name: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum JobKind {
    Ingest,
    Dataset,
}
