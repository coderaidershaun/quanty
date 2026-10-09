//! The `quanty-mcp` command: serves quanty's tools to an AI agent over the standard input and
//! output, or over HTTP on this machine.

use std::net::Ipv4Addr;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use mcp::{PaidServices, QuantyServer, serve_http, serve_stdio};
use rag_core::Config;
use tokio::net::TcpListener;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser)]
#[command(name = "quanty-mcp")]
struct Cli {
    /// Serve over HTTP at http://127.0.0.1:PORT/mcp, on this machine only, with no login. Without
    /// this the server talks over its standard input and output
    #[arg(long, value_name = "PORT")]
    http: Option<u16>,
}

#[tokio::main]
async fn main() -> ExitCode {
    // The standard output belongs to the protocol while the server talks over it, so every log
    // line goes to the standard error. `rag_core` logs what each `claude` question used.
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            Targets::new()
                .with_default(Level::WARN)
                .with_target("rag_core", Level::INFO)
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

async fn run(cli: Cli) -> Result<()> {
    let config = Config::load().context("could not read the settings")?;
    let server = QuantyServer::new(config, PaidServices);
    match cli.http {
        None => {
            tracing::info!("serving over the standard input and output");
            serve_stdio(server).await.context("the server stopped")
        }
        Some(port) => {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
                .await
                .with_context(|| format!("could not listen on 127.0.0.1:{port}"))?;
            let address = listener
                .local_addr()
                .context("could not read the address that the server listens on")?;
            tracing::info!("listening on http://{address}/mcp");
            serve_http(server, listener)
                .await
                .context("the server stopped")
        }
    }
}
