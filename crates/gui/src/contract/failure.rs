//! What went wrong, in words a person can act on, and the three-state value that holds a result
//! that is still on its way.

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Failure {
    pub kind: FailureKind,
    /// What the person must do, in one sentence, with the value that matters.
    pub hint: String,
    /// The error and its causes on one line, for a "Details" view.
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FailureKind {
    QdrantDown,
    FalkorDbDown,
    EmbeddingKeyMissing,
    ConverterKeyMissing,
    EmbeddingFailed,
    ClaudeMissing,
    ClaudeSignedOut,
    ClaudeUsageLimit,
    ClaudeApiKeySet,
    PopplerMissing,
    TimedOut,
    BadFile,
    PageFailed,
    ChapterTaken,
    SourceMissing,
    Internal,
}

impl FailureKind {
    /// The one home of the standard hint of each kind. Every place that builds a failure starts
    /// from it and adds a value with `Failure::with_hint` when one matters.
    pub fn hint(self) -> &'static str {
        match self {
            FailureKind::QdrantDown => "Start Qdrant, then try again.",
            FailureKind::FalkorDbDown => "Start FalkorDB, then try again.",
            FailureKind::EmbeddingKeyMissing => {
                "Add EMBEDDING_GEMINI_API_KEY to your .env and start quanty again."
            }
            FailureKind::ConverterKeyMissing => {
                "Add CONVERTER_JEV_API_KEY to your .env and start quanty again."
            }
            FailureKind::EmbeddingFailed => {
                "The embedding service did not answer. Check the network and try again."
            }
            FailureKind::ClaudeMissing => "Install the claude command, or add its folder to PATH.",
            FailureKind::ClaudeSignedOut => "Run claude in a terminal and sign in.",
            FailureKind::ClaudeUsageLimit => {
                "Wait until the limit resets, then try again. Search still works."
            }
            FailureKind::ClaudeApiKeySet => {
                "Quit quanty, unset ANTHROPIC_API_KEY, and start it again."
            }
            FailureKind::PopplerMissing => {
                "Install Poppler (brew install poppler) and start quanty again."
            }
            FailureKind::TimedOut => "It took too long. Try again.",
            FailureKind::BadFile => "Choose a PDF named chapter-<number>-<name>.pdf.",
            FailureKind::PageFailed => {
                "A page could not be converted. Start again: the pages already done are kept."
            }
            FailureKind::ChapterTaken => {
                "Another PDF is already converted as this book and chapter. Change the book title, or remove that chapter's folder."
            }
            FailureKind::SourceMissing => {
                "The chapter's files are not where they were. Ingest the chapter again."
            }
            FailureKind::Internal => {
                "Something went wrong inside quanty. Try again; if it stays, rest the pointer on this message to read the error."
            }
        }
    }
}

impl Failure {
    /// A failure of this kind, with the standard hint.
    pub fn new(kind: FailureKind, detail: impl Into<String>) -> Failure {
        Failure {
            kind,
            hint: kind.hint().to_owned(),
            detail: detail.into(),
        }
    }

    /// The same failure with a hint that has a value in it: an address, a page, a file name.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Failure {
        self.hint = hint.into();
        self
    }

    pub fn internal(detail: impl Into<String>) -> Failure {
        Failure::new(FailureKind::Internal, detail)
    }

    /// What a part that is not built yet answers, so the app shows a designed error state and
    /// not a wait.
    pub fn not_built(what: &str) -> Failure {
        Failure::internal(format!("{what} is not built yet"))
            .with_hint("This part of quanty is not built yet.")
    }
}

/// A result that is not there yet, is on its way, has arrived, or has failed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Loadable<T> {
    Idle,
    Loading,
    Ready(T),
    Failed(Failure),
}

// A derive would demand `T: Default`, and a result type should not have to be default-able.
#[allow(clippy::derivable_impls)]
impl<T> Default for Loadable<T> {
    fn default() -> Self {
        Loadable::Idle
    }
}

impl<T> Loadable<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Loadable::Ready(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Loadable::Loading)
    }

    pub fn failure(&self) -> Option<&Failure> {
        match self {
            Loadable::Failed(failure) => Some(failure),
            _ => None,
        }
    }
}

impl<T> From<Result<T, Failure>> for Loadable<T> {
    fn from(result: Result<T, Failure>) -> Self {
        match result {
            Ok(value) => Loadable::Ready(value),
            Err(failure) => Loadable::Failed(failure),
        }
    }
}
