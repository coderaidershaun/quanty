//! The stand-in key checks: services that answer the check of a Gemini key or a Jev key as the
//! test chose, and never reach a provider.

use std::time::Duration;

use backend::live::Services;
use contract::Failure;
use graph::FalkorGraph;
use ocr::convert::services::{Jev, JevError};
use ocr::{ChapterJob, ConversionSummary, ConvertError, PageProgress};
use rag_core::{ApiKey, Config, EmbedError, GeminiEmbedder, WorkKind};

/// How a key check answers.
#[derive(Debug, Clone, Copy)]
pub enum Check {
    Accepted,
    Rejected {
        status: u16,
        body: &'static str,
    },
    /// The real client runs against a port that nothing listens on.
    Unreachable,
}

/// `inner` does all the work except the two key checks, which answer as the test chose. No check
/// ever reaches a provider.
pub struct KeyChecks<S> {
    pub inner: S,
    pub gemini: Check,
    pub jev: Check,
}

const CLOSED_PORT: &str = "http://127.0.0.1:1";

impl<S: Services> Services for KeyChecks<S> {
    type Embedder = S::Embedder;
    type Llm = S::Llm;

    fn embedder(&self, config: &Config) -> Result<S::Embedder, Failure> {
        self.inner.embedder(config)
    }

    fn llm(&self, work: WorkKind) -> S::Llm {
        self.inner.llm(work)
    }

    fn graph(&self, config: &Config) -> impl Future<Output = Result<FalkorGraph, Failure>> + Send {
        self.inner.graph(config)
    }

    fn convert(
        &self,
        job: &ChapterJob,
        jev_api_key: Option<&str>,
        on_page: &mut (dyn FnMut(PageProgress) + Send),
    ) -> impl Future<Output = Result<ConversionSummary, ConvertError>> + Send {
        self.inner.convert(job, jev_api_key, on_page)
    }

    async fn check_gemini_key(&self, key: ApiKey) -> Result<(), EmbedError> {
        match self.gemini {
            Check::Accepted => Ok(()),
            Check::Rejected { status, body } => Err(EmbedError::Rejected {
                status,
                body: body.to_owned(),
            }),
            Check::Unreachable => {
                let embedder = GeminiEmbedder::new(&key)?;
                embedder
                    .with_endpoint(CLOSED_PORT, Duration::ZERO)
                    .check_key()
                    .await
            }
        }
    }

    async fn check_jev_key(&self, key: ApiKey) -> Result<(), JevError> {
        match self.jev {
            Check::Accepted => Ok(()),
            Check::Rejected { status, body } => Err(JevError::Rejected {
                status,
                body: body.to_owned(),
            }),
            Check::Unreachable => {
                let jev = Jev::new(key.expose())?;
                jev.with_base_url(CLOSED_PORT).check_key().await
            }
        }
    }
}
