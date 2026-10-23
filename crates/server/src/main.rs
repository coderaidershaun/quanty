//! The quanty command starts one home and serves its library to a browser.

use anyhow::Context;
use backend::{
    Handler,
    fake::{self, Fake},
    live::{LiveContext, RealServices},
};
use clap::Parser;
use mcp::{BackgroundJobs, PaidServices, QuantyServer};
use rag_core::Config;
use server::{Settings, launch};
use std::{net::SocketAddr, path::PathBuf};
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "quanty", about = "Ask your library in the browser.")]
struct Args {
    /// Listen on this IP address and port, overriding QUANTY_LISTEN.
    #[arg(long)]
    listen: Option<SocketAddr>,
    /// Folder containing .env, content and data. Defaults to the current folder.
    #[arg(long)]
    home: Option<PathBuf>,
    /// Run a built-in scene without stores or models, or print `list`.
    #[arg(long)]
    fixture: Option<String>,
    /// Open the server address in a browser.
    #[arg(long)]
    open: bool,
}

fn main() -> anyhow::Result<()> {
    // SAFETY: no threads exist before the PATH is set at process startup.
    unsafe { std::env::set_var("PATH", launch::search_path()) };
    let args = Args::parse();
    if args.fixture.as_deref() == Some("list") {
        for scene in fake::scenes() {
            println!("{:<16} {}", scene.name, scene.about);
        }
        return Ok(());
    }
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            Targets::new()
                .with_default(tracing::Level::WARN)
                .with_target("rag_core", tracing::Level::INFO)
                .with_target("backend", tracing::Level::INFO)
                .with_target("server", tracing::Level::INFO),
        )
        .init();
    let home = std::path::absolute(
        args.home
            .unwrap_or(std::env::current_dir().context("could not read the current folder")?),
    )
    .context("could not resolve the home folder")?;
    std::env::set_current_dir(&home)
        .with_context(|| format!("could not enter {}", home.display()))?;
    let mut settings = Settings::from_sources(&home, |name| std::env::var(name).ok())
        .context("could not read server settings")?;
    if let Some(listen) = args.listen {
        settings.listen = listen;
    }
    settings.validate().context("could not start quanty")?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("could not start the server runtime")?;
    runtime.block_on(async move {
        match args.fixture {
            Some(scene) => {
                let background_jobs = BackgroundJobs::new();
                let fake = Fake::scene(&scene, &home).context("could not start the fixture")?;
                let samples =
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/content");
                let options = server::ServerOptions {
                    settings,
                    content: samples,
                    fixture: Some(scene),
                    mcp: axum::Router::new(),
                    background_jobs,
                };
                serve(fake, options, args.open).await
            }
            None => {
                let background_jobs = BackgroundJobs::new();
                let config = Config::load().context("could not read the library settings")?;
                let content = config.content_folder.clone();
                let live =
                    LiveContext::new(config.clone(), RealServices::default()).with_home(home);
                let options = server::ServerOptions {
                    settings,
                    content,
                    fixture: None,
                    mcp: mcp::http_service(QuantyServer::with_background_jobs(
                        config,
                        PaidServices::default(),
                        background_jobs.clone(),
                    )),
                    background_jobs,
                };
                serve(live, options, args.open).await
            }
        }
    })
}

async fn serve<H: Handler>(
    handler: H,
    mut options: server::ServerOptions,
    open: bool,
) -> anyhow::Result<()> {
    let settings = options.settings.clone();
    let listener = tokio::net::TcpListener::bind(settings.listen)
        .await
        .with_context(|| format!("could not listen on {}", settings.listen))?;
    let address = listener
        .local_addr()
        .context("could not read the listening address")?;
    options.settings.listen = address;
    let app = server::router(handler, options).context("could not prepare server routes")?;
    let url = format!("http://{address}");
    println!("quanty is listening on {url}");
    if open {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "explorer"
        } else {
            "xdg-open"
        };
        if let Err(error) = std::process::Command::new(program).arg(&url).spawn() {
            tracing::warn!(%error, "could not open the browser");
        }
    }
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("the HTTP server stopped")
}
