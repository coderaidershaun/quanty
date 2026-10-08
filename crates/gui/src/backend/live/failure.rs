//! Turns each error of the backend into a `Failure`: the kind a person can act on, a hint with
//! the value that matters, and the whole chain of causes as the detail.
//!
//! Every `match` here ends with a catch-all arm. An error type that gains a variant still
//! compiles, and the new variant is `Internal` until it is given a row.

use std::error::Error;
use std::fmt::Display;
use std::path::Path;

use graph::GraphError;
use ocr::convert::services::{ClaudeError, JevError, ServiceError};
use ocr::{ContentError, ConvertError, PopplerError, ReadChapterError};
use rag_core::{ClaudeCliError, ConfigError, EmbedError, LlmError, StoreError};
use rag_ingestion::{ConceptError, DeleteError, IngestError, PdfError, RelabelError};
use rag_retrieval::{AnswerError, SearchError};

use crate::contract::{Failure, FailureKind as Kind};

fn chain(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}

/// With no hint the kind's standard one is used.
struct Verdict {
    kind: Kind,
    hint: Option<String>,
}

impl Verdict {
    fn of(kind: Kind) -> Verdict {
        Verdict { kind, hint: None }
    }

    fn saying(kind: Kind, hint: String) -> Verdict {
        let hint = Some(hint);
        Verdict { kind, hint }
    }

    fn internal() -> Verdict {
        Verdict::of(Kind::Internal)
    }

    fn failure(self, error: &dyn Error) -> Failure {
        let failure = Failure::new(self.kind, chain(error));
        match self.hint {
            Some(hint) => failure.with_hint(hint),
            None => failure,
        }
    }
}

fn timed_out(seconds: u64) -> Verdict {
    let hint = format!("It took longer than {seconds} seconds. Try again.");
    Verdict::saying(Kind::TimedOut, hint)
}

fn source_missing(place: &Path) -> Verdict {
    let place = place.display();
    let hint =
        format!("The document's files are not where they were ({place}). Ingest its PDF again.");
    Verdict::saying(Kind::SourceMissing, hint)
}

fn bad_file(file: &Path) -> Verdict {
    let name = file.file_name().map_or_else(
        || file.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let hint = format!("Choose a PDF that opens in a PDF reader. {name} is not one.");
    Verdict::saying(Kind::BadFile, hint)
}

fn bad_file_name(name: &str) -> Verdict {
    let hint = format!(
        "A book chapter's PDF must be named chapter-<number>-<name>.pdf, for example chapter-3-greeks.pdf. Rename {name}."
    );
    Verdict::saying(Kind::BadFile, hint)
}

fn unknown_document(id: &impl Display) -> Verdict {
    let hint =
        format!("Nothing is stored under the document id {id}. Use the id that rag-ingest prints.");
    Verdict::saying(Kind::SourceMissing, hint)
}

fn store(error: &StoreError) -> Verdict {
    match error {
        StoreError::Connect { url, .. }
        | StoreError::Unreachable { url, .. }
        | StoreError::Request { url, .. } => Verdict::saying(
            Kind::QdrantDown,
            format!("Start Qdrant at {url}, then try again."),
        ),
        _ => Verdict::internal(),
    }
}

// SMELL: a server that answers at the address but is not FalkorDB has no row here, so the person
// is told that something went wrong inside quanty. They could act on it: another program has
// that port.
fn graph(error: &GraphError) -> Verdict {
    match error {
        GraphError::Connect { url, .. }
        | GraphError::Ping { url, .. }
        | GraphError::Query { url, .. } => Verdict::saying(
            Kind::FalkorDbDown,
            format!("Start FalkorDB at {url}, then try again."),
        ),
        _ => Verdict::internal(),
    }
}

fn embed(error: &EmbedError) -> Verdict {
    match error {
        EmbedError::MissingApiKey => Verdict::of(Kind::EmbeddingKeyMissing),
        EmbedError::Transport(_) | EmbedError::Rejected { .. } => {
            Verdict::of(Kind::EmbeddingFailed)
        }
        _ => Verdict::internal(),
    }
}

fn llm(error: &LlmError) -> Verdict {
    match error {
        LlmError::ApiKeySet => Verdict::of(Kind::ClaudeApiKeySet),
        LlmError::Start(_) => Verdict::of(Kind::ClaudeMissing),
        LlmError::NotSignedIn { .. } => Verdict::of(Kind::ClaudeSignedOut),
        LlmError::UsageLimit { .. } => Verdict::of(Kind::ClaudeUsageLimit),
        LlmError::TimedOut { seconds } => timed_out(*seconds),
        _ => Verdict::internal(),
    }
}

fn claude_cli(error: &ClaudeCliError) -> Verdict {
    match error {
        ClaudeCliError::Start(_) => Verdict::of(Kind::ClaudeMissing),
        ClaudeCliError::NotSignedIn { .. } => Verdict::of(Kind::ClaudeSignedOut),
        ClaudeCliError::TimedOut { seconds } => timed_out(*seconds),
        #[allow(unreachable_patterns, reason = "every variant has a row today")]
        _ => Verdict::internal(),
    }
}

fn claude(error: &ClaudeError) -> Verdict {
    match error {
        ClaudeError::ApiKeySet => Verdict::of(Kind::ClaudeApiKeySet),
        ClaudeError::Spawn(_) => Verdict::of(Kind::ClaudeMissing),
        ClaudeError::TimedOut { seconds } => timed_out(*seconds),
        _ => Verdict::internal(),
    }
}

fn service(error: &ServiceError) -> Verdict {
    match error {
        ServiceError::Claude(error) => claude(error),
        ServiceError::Jev(JevError::MissingApiKey) => Verdict::of(Kind::ConverterKeyMissing),
        _ => Verdict::internal(),
    }
}

fn poppler(error: &PopplerError) -> Verdict {
    match error {
        PopplerError::Start { tool, .. } => {
            let hint = format!(
                "Install Poppler (brew install poppler), which provides {tool}, and start quanty again."
            );
            Verdict::saying(Kind::PopplerMissing, hint)
        }
        PopplerError::TimedOut { tool } => {
            Verdict::saying(Kind::TimedOut, format!("{tool} took too long. Try again."))
        }
        PopplerError::Failed { file, .. }
        | PopplerError::NoPageCount { file }
        | PopplerError::NoPageSize { file } => bad_file(file),
        #[allow(unreachable_patterns, reason = "every variant has a row today")]
        _ => Verdict::internal(),
    }
}

fn content(error: &ContentError) -> Verdict {
    match error {
        ContentError::BadFileName { name } => bad_file_name(name),
        // The file is fine here: it is a title that cannot name a folder.
        ContentError::EmptyMediaFolderName { title } => {
            let hint = format!(
                "The media title {title:?} has no letter or digit to name its folder with. Change the media title."
            );
            Verdict::saying(Kind::BadFile, hint)
        }
        ContentError::EmptyDocumentFolderName { title } => {
            let hint = format!(
                "The document title {title:?} has no letter or digit to name its folder with. Change the title."
            );
            Verdict::saying(Kind::BadFile, hint)
        }
        ContentError::Read { path, .. } | ContentError::Parse { path, .. } => source_missing(path),
        _ => Verdict::internal(),
    }
}

fn read_chapter(error: &ReadChapterError) -> Verdict {
    match error {
        ReadChapterError::Content(error) => content(error),
        ReadChapterError::NotFinished { folder: place }
        | ReadChapterError::PieceFile { path: place, .. } => source_missing(place),
        _ => Verdict::internal(),
    }
}

fn convert(error: &ConvertError) -> Verdict {
    match error {
        ConvertError::Content(error) => content(error),
        ConvertError::ApiKeySet => Verdict::of(Kind::ClaudeApiKeySet),
        ConvertError::SourceUnreadable { path, .. } => bad_file(path),
        ConvertError::DifferentSource { folder, .. } => {
            let folder = folder.display();
            let hint = format!(
                "Another PDF is already converted as this document of this media in {folder}. Choose another media, or remove that folder."
            );
            Verdict::saying(Kind::ChapterTaken, hint)
        }
        ConvertError::Poppler(error) => poppler(error),
        ConvertError::Services(error) | ConvertError::ImageCallFailed { source: error, .. } => {
            service(error)
        }
        ConvertError::PageFailed { position, .. } => {
            let hint = format!(
                "Page {position} could not be converted. Start again: the pages already done are kept."
            );
            Verdict::saying(Kind::PageFailed, hint)
        }
        _ => Verdict::internal(),
    }
}

fn concepts(error: &ConceptError) -> Verdict {
    match error {
        // A usage limit during an ingest must not show as a generic failure: the same job run
        // again goes on from where it stopped.
        ConceptError::Stopped { source, .. } => llm(source),
        ConceptError::Embed { source, .. } => embed(source),
        ConceptError::ConceptStore(source) | ConceptError::RelatedItems { source, .. } => {
            store(source)
        }
        ConceptError::Graph(source) => graph(source),
        _ => Verdict::internal(),
    }
}

fn ingest(error: &IngestError) -> Verdict {
    match error {
        IngestError::ChapterFolder { path, .. } => source_missing(path),
        IngestError::Read(error) => read_chapter(error),
        IngestError::ModelNotReady(error) => llm(error),
        IngestError::Embed(error) => embed(error),
        IngestError::Store(error) => store(error),
        IngestError::Graph(error) => graph(error),
        IngestError::Concepts(error) => concepts(error),
        _ => Verdict::internal(),
    }
}

fn pdf(error: &PdfError) -> Verdict {
    match error {
        PdfError::Convert(error) => convert(error),
        PdfError::Graph(error) => graph(error),
        PdfError::Store(error) => store(error),
        PdfError::Ingest(error) => ingest(error),
        #[allow(unreachable_patterns, reason = "every variant has a row today")]
        _ => Verdict::internal(),
    }
}

fn delete(error: &DeleteError) -> Verdict {
    match error {
        DeleteError::Store(error) => store(error),
        DeleteError::Graph(error) => graph(error),
        DeleteError::UnknownDocument { id, .. } => unknown_document(id),
        #[allow(unreachable_patterns, reason = "every variant has a row today")]
        _ => Verdict::internal(),
    }
}

fn relabel(error: &RelabelError) -> Verdict {
    match error {
        RelabelError::Graph(error) => graph(error),
        RelabelError::Store(error) => store(error),
        RelabelError::UnknownDocument { id } => unknown_document(id),
        RelabelError::UnknownMedia { title } => {
            let hint = format!(
                "The library has no media titled {title:?}. Refresh the Library and choose a media that it lists."
            );
            Verdict::saying(Kind::SourceMissing, hint)
        }
        #[allow(unreachable_patterns, reason = "every variant has a row today")]
        _ => Verdict::internal(),
    }
}

fn search(error: &SearchError) -> Verdict {
    match error {
        SearchError::Embed(error) => embed(error),
        SearchError::Items(error) | SearchError::Concepts(error) => store(error),
        SearchError::Graph(error) => graph(error),
        #[allow(unreachable_patterns, reason = "every variant has a row today")]
        _ => Verdict::internal(),
    }
}

fn answer(error: &AnswerError) -> Verdict {
    match error {
        AnswerError::Llm(error) => llm(error),
        _ => Verdict::internal(),
    }
}

/// A settings error is raised before the window opens, so it has no row.
fn config(_error: &ConfigError) -> Verdict {
    Verdict::internal()
}

macro_rules! failures_from {
    ($($error:ty => $verdict:ident),+ $(,)?) => {
        $(impl From<$error> for Failure {
            fn from(error: $error) -> Failure {
                $verdict(&error).failure(&error)
            }
        })+
    };
}

failures_from!(
    StoreError => store,
    GraphError => graph,
    EmbedError => embed,
    LlmError => llm,
    ClaudeCliError => claude_cli,
    SearchError => search,
    AnswerError => answer,
    IngestError => ingest,
    PdfError => pdf,
    DeleteError => delete,
    RelabelError => relabel,
    ConceptError => concepts,
    ConvertError => convert,
    ContentError => content,
    ReadChapterError => read_chapter,
    ConfigError => config,
);
