//! The six things the app needs before it can work, what is known about each, and which
//! failures name one of them.

use super::failure::{Failure, FailureKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum ServiceState {
    #[default]
    Unknown,
    Checking,
    Up {
        detail: String,
    },
    Down(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct StartupFacts {
    pub home: std::path::PathBuf,
    pub env_file: Option<std::path::PathBuf>,
    /// The name of the scene when the app runs from fixtures.
    pub fixture: Option<String>,
    pub anthropic_api_key_set: bool,
}

impl Failure {
    /// A slow or failed call names no service: the service may be fine.
    pub fn service(&self) -> Option<Service> {
        match self.kind {
            FailureKind::QdrantDown => Some(Service::Qdrant),
            FailureKind::FalkorDbDown => Some(Service::FalkorDb),
            FailureKind::EmbeddingKeyMissing => Some(Service::EmbeddingKey),
            FailureKind::ConverterKeyMissing => Some(Service::ConverterKey),
            FailureKind::ClaudeMissing
            | FailureKind::ClaudeSignedOut
            | FailureKind::ClaudeUsageLimit
            | FailureKind::ClaudeApiKeySet => Some(Service::Claude),
            FailureKind::PopplerMissing => Some(Service::Poppler),
            FailureKind::EmbeddingFailed
            | FailureKind::TimedOut
            | FailureKind::BadFile
            | FailureKind::PageFailed
            | FailureKind::ChapterTaken
            | FailureKind::BookExists
            | FailureKind::SourceMissing
            | FailureKind::Internal => None,
        }
    }
}
