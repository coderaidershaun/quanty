//! Checks that Qdrant, FalkorDB and the sign-in of the `claude` program are ready. Every check
//! is always run, so one report names everything that is wrong.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use graph::{FalkorGraph, GraphError};
use rag_core::{ClaudeCliError, Config, ItemStore, StoreError, check_signed_in};

const STORE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(thiserror::Error, Debug)]
enum HealthError {
    #[error(transparent)]
    Qdrant(#[from] StoreError),

    #[error(transparent)]
    FalkorDb(#[from] GraphError),

    #[error(transparent)]
    Claude(#[from] ClaudeCliError),

    #[error("no answer within {seconds} seconds")]
    TimedOut { seconds: u64 },
}

/// `detail` says what was checked: the address of a store, or the sign-in state of claude.
#[derive(Debug)]
struct Check {
    name: &'static str,
    detail: String,
    result: Result<(), HealthError>,
}

impl fmt::Display for Check {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.result {
            Ok(()) => write!(formatter, "{}: ok ({})", self.name, self.detail),
            Err(error) => write!(
                formatter,
                "{}: FAILED ({}): {}",
                self.name,
                self.detail,
                one_line_chain(error)
            ),
        }
    }
}

#[derive(Debug)]
pub struct HealthReport {
    checks: [Check; 3],
}

impl HealthReport {
    pub fn is_healthy(&self) -> bool {
        self.checks.iter().all(|check| check.result.is_ok())
    }
}

/// One line for each check, with no line break after the last.
impl fmt::Display for HealthReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (position, check) in self.checks.iter().enumerate() {
            if position > 0 {
                writeln!(formatter)?;
            }
            write!(formatter, "{check}")?;
        }
        Ok(())
    }
}

/// It never fails: a check that fails is a line of the report.
pub async fn check(config: &Config) -> HealthReport {
    let qdrant = within_store_timeout(async {
        ItemStore::connect(config)?.ping().await?;
        Ok(())
    })
    .await;
    let falkordb = within_store_timeout(async {
        FalkorGraph::connect(config).await?.ping().await?;
        Ok(())
    })
    .await;
    let claude = check_signed_in().await.map_err(HealthError::from);

    HealthReport {
        checks: [
            Check {
                name: "Qdrant",
                detail: config.qdrant_url.clone(),
                result: qdrant,
            },
            Check {
                name: "FalkorDB",
                detail: config.falkordb_url.clone(),
                result: falkordb,
            },
            Check {
                name: "claude",
                detail: if claude.is_ok() {
                    "signed in"
                } else {
                    "not signed in"
                }
                .to_owned(),
                result: claude,
            },
        ],
    }
}

async fn within_store_timeout(
    check: impl Future<Output = Result<(), HealthError>>,
) -> Result<(), HealthError> {
    tokio::time::timeout(STORE_TIMEOUT, check)
        .await
        .unwrap_or(Err(HealthError::TimedOut {
            seconds: STORE_TIMEOUT.as_secs(),
        }))
}

fn one_line_chain(error: &dyn Error) -> String {
    let mut messages = vec![error.to_string()];
    let mut cause = error.source();
    while let Some(next) = cause {
        messages.push(next.to_string());
        cause = next.source();
    }
    messages
        .join(": ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
