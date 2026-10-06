//! The `rag-query` command: asks the stored items a question, or asks the golden questions and
//! scores them.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use rag_core::{Config, GeminiEmbedder, ItemKind, ItemStore};
use rag_retrieval::{Retriever, evaluate, read_golden_questions};
use tracing_subscriber::filter::LevelFilter;

const RESULTS_SHOWN: usize = 5;
const GOLDEN_FILE: &str = "golden.toml";

#[derive(Parser)]
#[command(name = "rag-query", args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// The question to ask
    question: Option<String>,

    /// Look only at items of this kind
    // SMELL: `rag-query --kind <kind> eval` is not refused. After an option the word `eval` is
    // taken as the question, so it is searched for and the golden questions are not asked.
    #[arg(long)]
    kind: Option<ItemKind>,
}

#[derive(Subcommand)]
enum Command {
    /// Ask every question in golden.toml and print how many are found in the top results
    Eval,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::WARN)
        .with_writer(std::io::stderr)
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
    match (cli.command, cli.question) {
        (Some(Command::Eval), _) => eval().await.map(|()| ExitCode::SUCCESS),
        (None, Some(question)) => ask(&question, cli.kind).await.map(|()| ExitCode::SUCCESS),
        (None, None) => {
            Cli::command()
                .print_help()
                .context("could not print the help text")?;
            Ok(ExitCode::from(2))
        }
    }
}

fn retriever() -> Result<Retriever<GeminiEmbedder>> {
    let config = Config::load().context("could not read the settings")?;
    let embedder = GeminiEmbedder::from_config(&config).context("could not set up the embedder")?;
    let store = ItemStore::connect(&config).context("could not set up the item store")?;
    Ok(Retriever::new(embedder, store))
}

async fn ask(question: &str, kind: Option<ItemKind>) -> Result<()> {
    let results = retriever()
        .context("could not get ready to search")?
        .search(question, kind, RESULTS_SHOWN)
        .await
        .context("could not search for the question")?;
    println!("{results}");
    Ok(())
}

async fn eval() -> Result<()> {
    let golden = read_golden_questions(Path::new(GOLDEN_FILE)).with_context(|| {
        format!(
            "could not load {GOLDEN_FILE} from the current folder; run eval from the workspace root"
        )
    })?;
    let retriever = retriever().context("could not get ready to search")?;
    let report = evaluate(&golden, &retriever)
        .await
        .context("could not ask the golden questions")?;
    println!("{report}");
    Ok(())
}
