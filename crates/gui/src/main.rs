//! The `quanty` command: opens the desktop app.

use std::path::PathBuf;

use anyhow::Context;
use choices::{DATABASES_FILE, Databases, LOCAL_DATABASE};
use clap::Parser;
use gui::app;
use gui::app::launch::{self, Home};
use gui::backend::fake::{self, Fake};
use gui::backend::live::{LiveContext, RealServices};
use gui::contract::StartupFacts;
use rag_core::Config;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser)]
#[command(
    name = "quanty",
    about = "Ask your library. Answers come with the page they stand on."
)]
struct Args {
    /// Run from built-in fixtures, with no store and no model: the name of a scene, or `list`.
    #[arg(long, value_name = "SCENE")]
    fixture: Option<String>,
    /// The folder that holds `.env`, `content/` and `data/`. Default: found from where you are.
    #[arg(long, value_name = "FOLDER")]
    home: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    // SAFETY: this is the first statement of the program. No other thread exists yet, so
    // nothing reads the environment while it changes.
    unsafe { std::env::set_var("PATH", launch::search_path()) };
    let args = Args::parse();
    if args.fixture.as_deref() == Some("list") {
        for scene in fake::scenes() {
            println!("{:<16} {}", scene.name, scene.about);
        }
        return Ok(());
    }
    log_to_stderr();
    let home =
        launch::find_home(args.home.as_deref()).context("could not find the quanty home folder")?;
    std::env::set_current_dir(&home.folder)
        .with_context(|| format!("could not enter {}", home.folder.display()))?;
    let opened = match &args.fixture {
        // A fixture never reads the databases file, so it never names a real server.
        Some(scene) => {
            let fake = Fake::scene(scene, &home.folder).context("could not start the fixtures")?;
            let facts = home.facts(
                Some(scene),
                Fake::local_database(&home.folder),
                fake.databases(),
            );
            let opening = fake.opening();
            app::run(fake, facts, opening)
        }
        None => {
            let (live, facts) = live_settings(&home)?;
            app::run(
                LiveContext::new(live, RealServices::default()),
                facts,
                Vec::new(),
            )
        }
    };
    opened.map_err(|error| anyhow::anyhow!("could not open the window: {error}"))
}

/// The settings with the chosen database applied, and the facts that keep the saved databases
/// and Local for the sheet. While the environment names a store, Local is used, so the footer
/// never names a database that the app does not use.
fn live_settings(home: &Home) -> anyhow::Result<(Config, StartupFacts)> {
    let environment = |name: &str| std::env::var(name).ok();
    let base = Config::from_sources(environment, home.env_file.as_deref())
        .context("could not read the settings")?;
    let mut databases = Databases::load_from(&home.folder.join(DATABASES_FILE))
        .context("could not read the settings")?;
    if Config::environment_names_a_store(environment) {
        tracing::warn!(
            "the environment names a store, so the app uses Local and not the chosen database"
        );
        databases.chosen = Some(LOCAL_DATABASE.to_owned());
    }
    let live = base
        .clone()
        .with_chosen(&databases)
        .context("could not read the settings")?;
    let facts = home.facts(None, base.local_database(), databases);
    Ok((live, facts))
}

/// What the backend logs goes to the terminal, so a person sees what each model call used.
fn log_to_stderr() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            Targets::new()
                .with_default(Level::WARN)
                .with_target("rag_core", Level::INFO)
                .with_target("gui", Level::INFO),
        )
        .init();
}
