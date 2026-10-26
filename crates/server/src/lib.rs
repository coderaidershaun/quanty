//! One server gives browsers and agents access to the same library.

mod auth;
mod bundle;
mod content;
mod downloads;
mod jobs;
pub mod launch;
mod uploads;
mod ws;

use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, get, post},
};
use backend::Handler;
use contract::StartupFacts;
pub use launch::{Settings, SettingsError};
use mcp::BackgroundJobs;
use std::{path::PathBuf, sync::Arc, time::Duration};

/// The type of a file that the server sends, by its extension.
fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("pdf") => "application/pdf",
        Some("md") => "text/markdown; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error("could not clean old staged files: {0}")]
    Cleanup(#[source] std::io::Error),
}

/// Inputs shared by the browser routes and the agent endpoint.
pub struct ServerOptions {
    pub settings: Settings,
    pub content: PathBuf,
    pub fixture: Option<String>,
    pub mcp: Router,
    pub background_jobs: BackgroundJobs,
}

/// Serves a live or fixture backend through the same authenticated routes.
///
/// # Errors
/// Refuses unsafe settings or staging folders that cannot be cleaned.
pub fn router<H: Handler>(handler: H, options: ServerOptions) -> Result<Router, RouterError> {
    let ServerOptions {
        settings,
        content,
        fixture,
        mcp,
        background_jobs,
    } = options;
    settings.validate()?;
    for (folder, days) in [(backend::UPLOADS, 7), (backend::EXPORTS, 1)] {
        uploads::clean_old_folders(
            &settings.home.join(folder),
            Duration::from_secs(days * 24 * 60 * 60),
        )
        .map_err(RouterError::Cleanup)?;
    }
    let auth = auth::Auth::new(settings.clone());
    let guard = jobs::Guard::new(background_jobs);
    let facts_guard = guard.clone();
    let home = settings.home.clone();
    let sockets = ws::SocketState {
        handler: Arc::new(handler),
        guard,
    };
    let has_mcp =
        mcp.has_routes() && (settings.trusted_integration || settings.mcp_token.is_some());
    let protected = Router::new()
        .route(
            "/jet-app.json",
            get(move || async move {
                let mut descriptor = serde_json::json!({
                    "version": 1, "name": "Quanty", "ui": "./", "embedding": "relative-paths"
                });
                if has_mcp {
                    descriptor["mcp"] = "mcp".into();
                }
                Json(descriptor)
            }),
        )
        .route(
            "/api/facts",
            get(move || {
                let home = home.clone();
                let fixture = fixture.clone();
                let guard = facts_guard.clone();
                async move {
                    let model_choices =
                        choices::WorkChoices::load_from(&home.join(choices::MODEL_SETTINGS_FILE))
                            .map_err(|error| {
                            tracing::error!(%error, "could not load model choices");
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR
                        })?;
                    Ok::<_, axum::http::StatusCode>(Json(StartupFacts {
                        fixture,
                        anthropic_api_key_set: std::env::var_os("ANTHROPIC_API_KEY")
                            .is_some_and(|v| !v.is_empty()),
                        model_choices,
                        running: guard.running(),
                    }))
                }
            }),
        )
        .merge(
            Router::new()
                .route("/api/ws", get(ws::upgrade::<H>))
                .with_state(sockets),
        )
        .merge(
            Router::new()
                .route("/api/upload", post(uploads::upload))
                .layer(DefaultBodyLimit::max(settings.upload_limit))
                .route("/api/download/{id}/{file}", get(downloads::download))
                .with_state(settings),
        )
        .merge(
            Router::new()
                .route("/content/{*path}", get(content::picture))
                .with_state(content),
        )
        .route("/logout", post(auth::logout).with_state(auth.clone()))
        .fallback(bundle::serve)
        .layer(middleware::from_fn_with_state(auth.clone(), auth::protect));
    let mcp = if mcp.has_routes() {
        mcp.layer(middleware::from_fn_with_state(auth.clone(), auth::bearer))
    } else {
        Router::new().route(
            mcp::MCP_PATH,
            any(|| async { axum::http::StatusCode::NOT_FOUND }),
        )
    };
    Ok(protected
        .merge(mcp)
        .route(
            "/login",
            get(auth::login_page)
                .post(auth::login)
                .with_state(auth.clone()),
        )
        .route("/healthz", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(auth, auth::check_origin)))
}
