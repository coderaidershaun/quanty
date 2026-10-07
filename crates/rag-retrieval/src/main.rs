//! The `rag-query` command: asks the stored items a question and prints what it finds or an
//! answer written from it, or asks the golden questions and scores them.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use graph::FalkorGraph;
use rag_core::{
    ClaudeCli, ConceptStore, Config, DocumentLabels, GeminiEmbedder, ItemKind, ItemStore, Tag,
};
use rag_retrieval::{
    ANSWER_MODEL, Retriever, SearchResults, answer, evaluate, read_golden_questions,
};
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const GOLDEN_FILE: &str = "golden.toml";

#[derive(Parser)]
#[command(name = "rag-query", args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// The question to ask
    question: Option<String>,

    /// Look only at items of this kind
    // SMELL: `rag-query --kind <kind> eval` and `rag-query --answer eval` are not refused, and
    // neither is `eval` after `--book`, `--author` or `--tag`. After an option the word `eval` is
    // taken as the question, so it is searched for and the golden questions are not asked.
    #[arg(long)]
    kind: Option<ItemKind>,

    /// Look only at items of documents from this book, whatever its capitals
    #[arg(long)]
    book: Option<String>,

    /// Look only at items of documents by this author, whatever its capitals
    #[arg(long)]
    author: Option<String>,

    /// Look only at items of documents that have this tag. Repeat it to ask for more tags: a
    /// document must match every one of the book, the author and the tags that are given
    #[arg(long = "tag", value_name = "TAG")]
    tags: Vec<Tag>,

    /// Write an answer from the items found, with its sources
    #[arg(long)]
    answer: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Ask every question in golden.toml and print how many are found in the top results
    Eval,
}

#[tokio::main]
async fn main() -> ExitCode {
    // `rag_core` logs what each `claude` question cost, so a person sees what an answer cost as it
    // happens.
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            Targets::new()
                .with_default(LevelFilter::WARN)
                .with_target("rag_core", LevelFilter::INFO),
        )
        .init();
    match run(Cli::parse()).await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<ExitCode> {
    let wanted = DocumentLabels {
        book: cli.book,
        author: cli.author,
        tags: cli.tags.into_iter().collect(),
    };
    match (cli.command, cli.question) {
        (Some(Command::Eval), _) => eval().await.map(|()| ExitCode::SUCCESS),
        (None, Some(question)) if cli.answer => answer_question(&question, cli.kind, &wanted)
            .await
            .map(|()| ExitCode::SUCCESS),
        (None, Some(question)) => ask(&question, cli.kind, &wanted)
            .await
            .map(|()| ExitCode::SUCCESS),
        (None, None) => {
            Cli::command()
                .print_help()
                .context("could not print the help text")?;
            Ok(ExitCode::from(2))
        }
    }
}

async fn retriever() -> Result<Retriever<GeminiEmbedder, FalkorGraph>> {
    let config = Config::load().context("could not read the settings")?;
    // The graph is connected first, so a store that is down stops the command before anything is
    // embedded and billed.
    let graph = FalkorGraph::connect(&config)
        .await
        .context("could not connect to the graph")?;
    Ok(Retriever {
        embedder: GeminiEmbedder::from_config(&config).context("could not set up the embedder")?,
        items: ItemStore::connect(&config).context("could not set up the item store")?,
        concepts: ConceptStore::connect(&config).context("could not set up the concept store")?,
        graph,
    })
}

async fn search(
    question: &str,
    kind: Option<ItemKind>,
    wanted: &DocumentLabels,
) -> Result<SearchResults> {
    retriever()
        .await
        .context("could not get ready to search")?
        .search(question, kind, wanted)
        .await
        .context("could not search for the question")
}

async fn ask(question: &str, kind: Option<ItemKind>, wanted: &DocumentLabels) -> Result<()> {
    println!("{}", search(question, kind, wanted).await?);
    Ok(())
}

/// Prints the answer and nothing else. When nothing is found there is nothing to write an answer
/// from, so the model is not asked.
// SMELL: a `claude` that cannot start, because it is not signed in or `ANTHROPIC_API_KEY` is set,
// is found only after the search, so Gemini has billed the question by then.
async fn answer_question(
    question: &str,
    kind: Option<ItemKind>,
    wanted: &DocumentLabels,
) -> Result<()> {
    let results = search(question, kind, wanted).await?;
    if results.hits.is_empty() {
        println!("{results}");
        return Ok(());
    }
    let written = answer(&ClaudeCli::new(ANSWER_MODEL), question, &results)
        .await
        .context("could not write an answer")?;
    println!("{written}");
    Ok(())
}

async fn eval() -> Result<()> {
    let golden = read_golden_questions(Path::new(GOLDEN_FILE)).with_context(|| {
        format!(
            "could not load {GOLDEN_FILE} from the current folder; run eval from the workspace root"
        )
    })?;
    let retriever = retriever().await.context("could not get ready to search")?;
    let report = evaluate(&golden, &retriever)
        .await
        .context("could not ask the golden questions")?;
    println!("{report}");
    Ok(())
}
