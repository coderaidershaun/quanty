//! The four outside calls a page needs, behind one trait so the rest of the run can be tested
//! with stub answers. The live version retries only failures that are worth retrying.

use std::path::PathBuf;
use std::time::Duration;

use crate::categorise::{PageCategories, categorise_page_with_usage};
use crate::claude::{Answer, ClaudeError};
use crate::jev::{Jev, JevError, MathPlacement};
use crate::transcribe::{CopiedPage, TranscribedPage, copy_page, transcribe_page_with_picture};

const CLAUDE_RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(30), Duration::from_secs(120)];
const JEV_RETRY_DELAY: Duration = Duration::from_secs(2);
// A short line that is not blank, because a blank one never reaches the API.
const KEY_PROBE_TEXT: &str = "a + b = c";

/// One page of the chapter, already cut out and read.
#[derive(Debug, Clone)]
pub struct PageSource {
    /// 1 for the first page of the chapter.
    pub position: u32,
    /// The one-page PDF.
    pub pdf: PathBuf,
    /// A sharp picture of the page for the models to read. It is deleted when the page is saved.
    pub image: PathBuf,
    /// What `pdftotext -layout` printed for the page.
    pub text_layer: String,
}

/// Why an outside call failed.
#[derive(thiserror::Error, Debug)]
pub enum ServiceError {
    #[error(transparent)]
    Claude(#[from] ClaudeError),

    #[error(transparent)]
    Jev(#[from] JevError),
}

/// The paid calls made for a page. Each one means "ask until there is an answer or give up":
/// retrying is the implementation's job, not the caller's.
pub trait PageServices {
    /// Which kinds of content the page holds.
    fn tag(
        &self,
        page: &PageSource,
    ) -> impl Future<Output = Result<Answer<PageCategories>, ServiceError>> + Send;

    /// Where the math in the page's text layer sits, if there is any.
    fn contains_math(
        &self,
        page: &PageSource,
    ) -> impl Future<Output = Result<Option<MathPlacement>, ServiceError>> + Send;

    /// A plain copy of the page's text, or a flag that the page needs the stronger model.
    fn copy(
        &self,
        page: &PageSource,
    ) -> impl Future<Output = Result<Answer<CopiedPage>, ServiceError>> + Send;

    /// A full transcription of the page. `correction` says why the last one was rejected.
    fn transcribe(
        &self,
        page: &PageSource,
        correction: Option<&str>,
    ) -> impl Future<Output = Result<Answer<TranscribedPage>, ServiceError>> + Send;
}

/// Haiku and Sonnet through the `claude` command, and Jev over HTTP.
pub struct LiveServices {
    jev: Jev,
}

impl LiveServices {
    /// Builds the services and sends Jev one short line, so a rejected key stops the run before
    /// any page is started.
    ///
    /// # Errors
    /// - [`ServiceError::Jev`] if `CONVERTER_JEV_API_KEY` is not set or Jev refuses the key
    pub async fn from_env() -> Result<Self, ServiceError> {
        let services = Self {
            jev: Jev::from_env()?,
        };
        services.jev_contains_math(KEY_PROBE_TEXT).await?;
        Ok(services)
    }

    async fn jev_contains_math(&self, text: &str) -> Result<Option<MathPlacement>, JevError> {
        match self.jev.contains_math(text).await {
            Ok(placement) => Ok(placement),
            Err(_) => {
                tokio::time::sleep(JEV_RETRY_DELAY).await;
                self.jev.contains_math(text).await
            }
        }
    }
}

/// Runs `attempt` again after a pause when it fails in a way that is worth retrying. A usage
/// limit is never retried, however it arrives: asking again would only spend more of it.
async fn with_retries<T, Attempt, Pending>(mut attempt: Attempt) -> Result<T, ClaudeError>
where
    Attempt: FnMut() -> Pending,
    Pending: Future<Output = Result<T, ClaudeError>>,
{
    let mut delays = CLAUDE_RETRY_DELAYS.iter();
    loop {
        match attempt().await {
            Err(error) if error.is_worth_retrying() => match delays.next() {
                Some(delay) => tokio::time::sleep(*delay).await,
                None => return Err(error),
            },
            outcome => return outcome,
        }
    }
}

impl PageServices for LiveServices {
    async fn tag(&self, page: &PageSource) -> Result<Answer<PageCategories>, ServiceError> {
        Ok(with_retries(|| categorise_page_with_usage(&page.pdf)).await?)
    }

    async fn contains_math(
        &self,
        page: &PageSource,
    ) -> Result<Option<MathPlacement>, ServiceError> {
        Ok(self.jev_contains_math(&page.text_layer).await?)
    }

    async fn copy(&self, page: &PageSource) -> Result<Answer<CopiedPage>, ServiceError> {
        Ok(with_retries(|| copy_page(&page.pdf)).await?)
    }

    async fn transcribe(
        &self,
        page: &PageSource,
        correction: Option<&str>,
    ) -> Result<Answer<TranscribedPage>, ServiceError> {
        Ok(
            with_retries(|| transcribe_page_with_picture(&page.pdf, Some(&page.image), correction))
                .await?,
        )
    }
}
