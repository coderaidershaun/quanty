//! The `rag-dataset` command: builds a training dataset from the converted documents.

mod cli;

use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use graph::FalkorGraph;
use rag_core::{AgentCli, Config, UsageTally, WorkKind};
use rag_dataset::{GenerationKind, Step};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::cli::{Cli, Command, GivenBuild};

#[tokio::main]
async fn main() -> ExitCode {
    // `rag_core` logs what each model question used, and a person should see it as it happens.
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            Targets::new()
                .with_default(Level::WARN)
                .with_target("rag_core", Level::INFO)
                .with_target("rag_dataset", Level::INFO),
        )
        .init();
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Build(given) => build(&given).await,
    }
}

async fn build(given: &GivenBuild) -> Result<()> {
    let config = Config::load().context("could not read the settings")?;
    let options = given.options(&config);
    let llm = AgentCli::for_work(WorkKind::Datasets);
    let graph =
        if given.no_graph {
            None
        } else {
            Some(FalkorGraph::connect(&config).await.context(
                "could not connect to the graph; start it, or run again with --no-graph",
            )?)
        };
    let report = rag_dataset::build(&options, &llm, graph.as_ref(), &mut print_step)
        .await
        .context("could not build the dataset")?;
    println!("{report}");
    Ok(())
}

/// The steps go to standard error, so that standard output holds only the summary. The spending
/// so far is not printed, because the summary says what the whole run used.
fn print_step(step: Step, _spent: &UsageTally) {
    if let Some(line) = progress_line(step) {
        eprintln!("{line}");
    }
}

/// One line for each stage when it starts, never one for each document.
fn progress_line(step: Step) -> Option<String> {
    match step {
        Step::Exporting { done: 0, total } => Some(format!("exporting {total} documents")),
        Step::Checking => Some("checking the sections".to_owned()),
        Step::Planning => Some("planning the generation".to_owned()),
        Step::Writing {
            kind: GenerationKind::Synthetic,
            done: 0,
            total,
        } => Some(format!("writing {total} synthetic texts")),
        Step::Writing {
            kind: GenerationKind::Sft,
            done: 0,
            total,
        } => Some(format!("writing {total} SFT examples")),
        Step::WritingManifest => Some("writing the manifest".to_owned()),
        Step::Exporting { .. } | Step::Writing { .. } => None,
    }
}
