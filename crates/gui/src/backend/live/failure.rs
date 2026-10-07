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

/// The error and each cause under it, on one line.
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

/// What a person is told about an error: the kind, and a hint when a value belongs in it. With no
/// hint the kind's standard one is used.
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
        format!("The chapter's files are not where they were ({place}). Ingest the chapter again.");
    Verdict::saying(Kind::SourceMissing, hint)
}

fn bad_file(file: &Path) -> Verdict {
    let name = file.file_name().map_or_else(
        || file.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let hint = format!(
        "Choose a PDF named chapter-<number>-<name>.pdf that opens in a PDF reader. {name} is not one."
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
        ContentError::BadFileName { name } => bad_file(Path::new(name)),
        // The file is fine here: it is the book title that cannot name a folder.
        ContentError::EmptyBookFolderName { title } => {
            let hint = format!(
                "The book title {title:?} has no letter or digit to name its folder with. Change the book title."
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
                "Another PDF is already converted as this book and chapter in {folder}. Change the book title, or remove that chapter's folder."
            );
            Verdict::saying(Kind::ChapterTaken, hint)
        }
        ConvertError::Poppler(error) => poppler(error),
        ConvertError::Services(error) => service(error),
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

#[cfg(test)]
mod tests {
    use std::io;
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::ExitStatus;

    use graph::FalkorGraph;
    use ocr::PageError;
    use rag_core::{ApiKey, Config, DocId, ItemStore};

    use super::*;

    /// Settings whose addresses are not URLs, so a store refuses at once with no network.
    fn unusable_addresses() -> Config {
        let folder = std::env::temp_dir().join("quanty-failure-test");
        Config {
            qdrant_url: "not a url".to_owned(),
            falkordb_url: "not a url either".to_owned(),
            falkordb_graph: "test".to_owned(),
            items_collection: "test-items".to_owned(),
            concepts_collection: "test-concepts".to_owned(),
            concept_cache_folder: folder.join("cache"),
            concept_decision_log: folder.join("decisions.jsonl"),
            content_folder: folder.join("content"),
            gemini_api_key: Some(ApiKey::new("a-made-up-key")),
            jev_api_key: None,
        }
    }

    fn io() -> io::Error {
        io::Error::other("the disk said no")
    }

    fn p(text: &str) -> PathBuf {
        PathBuf::from(text)
    }

    fn limit() -> LlmError {
        LlmError::UsageLimit {
            message: "resets at 5pm".to_owned(),
        }
    }

    /// The failure for `error`, checked to carry the chain of causes on one line.
    fn told<E: Error + Into<Failure>>(error: E) -> Failure {
        let expected = chain(&error);
        let failure: Failure = error.into();
        assert_eq!(
            failure.detail, expected,
            "the detail is the chain of causes"
        );
        assert!(!failure.detail.contains('\n'), "the detail is one line");
        failure
    }

    /// The error has this kind and the standard hint of the kind.
    fn plain<E: Error + Into<Failure>>(error: E, kind: Kind) {
        let failure = told(error);
        assert_eq!((failure.kind, failure.hint.as_str()), (kind, kind.hint()));
    }

    /// The error has this kind, and its hint says the value.
    fn names<E: Error + Into<Failure>>(error: E, kind: Kind, value: &str) {
        let failure = told(error);
        assert_eq!(failure.kind, kind, "{failure:?}");
        assert!(
            failure.hint.contains(value),
            "{failure:?} must name {value}"
        );
        assert_ne!(failure.hint, kind.hint(), "{failure:?} must say more");
    }

    // One row to a line, so the table can be read down the page. rustfmt would turn every error
    // with fields into five lines.
    #[rustfmt::skip]
    #[test]
    fn every_backend_error_tells_the_person_what_to_do() {
        // The stores say their address.
        let config = unusable_addresses();
        names(ItemStore::connect(&config).err().unwrap(), Kind::QdrantDown, "not a url");
        let runtime = tokio::runtime::Runtime::new().expect("a runtime starts");
        let falkor = runtime.block_on(FalkorGraph::connect(&config)).err().unwrap();
        names(falkor, Kind::FalkorDbDown, "not a url either");

        // The keys and the models.
        plain(EmbedError::MissingApiKey, Kind::EmbeddingKeyMissing);
        plain(EmbedError::Rejected { status: 429, body: String::new() }, Kind::EmbeddingFailed);
        plain(LlmError::ApiKeySet, Kind::ClaudeApiKeySet);
        plain(LlmError::Start(io()), Kind::ClaudeMissing);
        plain(LlmError::NotSignedIn { message: String::new() }, Kind::ClaudeSignedOut);
        plain(limit(), Kind::ClaudeUsageLimit);
        names(LlmError::TimedOut { seconds: 90 }, Kind::TimedOut, "90 seconds");
        plain(ClaudeCliError::Start(io()), Kind::ClaudeMissing);
        let status = ExitStatus::from_raw(256);
        plain(ClaudeCliError::NotSignedIn { status, stderr: String::new() }, Kind::ClaudeSignedOut);
        names(ClaudeCliError::TimedOut { seconds: 10 }, Kind::TimedOut, "10 seconds");

        // An error that only wraps another takes the row of the one inside, and keeps the whole
        // chain as its detail.
        plain(SearchError::Embed(EmbedError::MissingApiKey), Kind::EmbeddingKeyMissing);
        plain(AnswerError::Llm(limit()), Kind::ClaudeUsageLimit);
        let stopped = || ConceptError::Stopped { read: 3, items: 9, source: limit() };
        plain(PdfError::Ingest(IngestError::Concepts(stopped())), Kind::ClaudeUsageLimit);
        let detail = told(IngestError::Concepts(stopped())).detail;
        assert!(detail.contains("usage limit"), "the cause is in the detail");

        // The converter.
        let poppler = ConvertError::Poppler;
        let services = ConvertError::Services;
        names(poppler(PopplerError::Start { tool: "pdftoppm", source: io() }), Kind::PopplerMissing, "pdftoppm");
        names(poppler(PopplerError::TimedOut { tool: "pdfinfo" }), Kind::TimedOut, "pdfinfo");
        names(poppler(PopplerError::NoPageCount { file: p("/books/chapter-1-a.pdf") }), Kind::BadFile, "chapter-1-a.pdf");
        plain(services(ServiceError::Jev(JevError::MissingApiKey)), Kind::ConverterKeyMissing);
        plain(services(ServiceError::Claude(ClaudeError::Spawn(io()))), Kind::ClaudeMissing);
        plain(ConvertError::ApiKeySet, Kind::ClaudeApiKeySet);
        names(ContentError::BadFileName { name: "notes.pdf".to_owned() }, Kind::BadFile, "notes.pdf");
        names(ContentError::EmptyBookFolderName { title: "?!".to_owned() }, Kind::BadFile, "book title \"?!\"");
        names(ConvertError::SourceUnreadable { path: p("/books/gone.pdf"), source: io() }, Kind::BadFile, "gone.pdf");
        let slow = PageError::Service(ServiceError::Claude(ClaudeError::TimedOut { seconds: 5 }));
        names(ConvertError::PageFailed { position: 4, folder: p("/work"), source: Box::new(slow) }, Kind::PageFailed, "Page 4");
        let (saved_file, given_file) = ("a.pdf".to_owned(), "b.pdf".to_owned());
        names(ConvertError::DifferentSource { folder: p("/content/ch-2"), saved_file, given_file }, Kind::ChapterTaken, "/content/ch-2");

        // A chapter, a document or a file that is not where it was.
        names(IngestError::ChapterFolder { path: p("/content/ch-3"), source: io() }, Kind::SourceMissing, "/content/ch-3");
        names(ContentError::Read { path: p("/content/chapter.json"), source: io() }, Kind::SourceMissing, "chapter.json");
        let bad_json = serde_json::from_str::<u32>("not json").expect_err("that is not json");
        names(ContentError::Parse { path: p("/content/page.json"), source: bad_json }, Kind::SourceMissing, "page.json");
        names(ReadChapterError::NotFinished { folder: p("/content/half") }, Kind::SourceMissing, "/content/half");
        names(ReadChapterError::PieceFile { path: p("/content/03-text.md"), source: io() }, Kind::SourceMissing, "03-text.md");
        let id: DocId = "00000000-0000-0000-0000-00000000002a".parse().expect("a document id");
        names(DeleteError::UnknownDocument { id, collection: "items".to_owned() }, Kind::SourceMissing, "2a");
        names(RelabelError::UnknownDocument { id }, Kind::SourceMissing, "2a");

        // Anything else is `Internal`, with the error text in the detail.
        plain(LlmError::Failed { exit_code: Some(1), reason: "odd".to_owned() }, Kind::Internal);
        plain(EmbedError::UnsupportedImage { path: p("a.gif") }, Kind::Internal);
        plain(ConfigError::DotenvParse { path: p(".env") }, Kind::Internal);
        plain(IngestError::VectorCount { items: 2, vectors: 1 }, Kind::Internal);
        plain(ContentError::Write { path: p("x"), source: io() }, Kind::Internal);
    }
}
