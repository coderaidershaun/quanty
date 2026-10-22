//! The `quanty-mcp` command: serves quanty's tools to an AI agent over the standard input and
//! output.

use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use mcp::{PaidServices, QuantyServer, serve_stdio};
use rag_core::Config;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser)]
#[command(name = "quanty-mcp")]
struct Cli {}

#[tokio::main]
async fn main() -> ExitCode {
    // The standard output belongs to the protocol, so every log goes to standard error.
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            Targets::new()
                .with_default(Level::WARN)
                .with_target("rag_core", Level::INFO)
                .with_target("rag_dataset", Level::INFO)
                .with_target("quanty_mcp", Level::INFO),
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

async fn run(_cli: Cli) -> Result<()> {
    let config = Config::load().context("could not read the settings")?;
    let server = QuantyServer::new(config, PaidServices::default());
    tracing::info!("serving over the standard input and output");
    serve_stdio(server).await.context("the server stopped")
}
