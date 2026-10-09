//! The four outside calls a page needs, and the one a picture that stands alone needs, each
//! behind a trait so the rest of the run can be tested with stub answers.

mod categorise;
mod claude;
mod jev;
mod schema;
mod transcribe;

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::reply::{CopiedPage, TranscribedPage};
use crate::content::PageCategories;

use categorise::categorise_page_with_usage;
use transcribe::{copy_page, transcribe_page_with_picture};

pub use categorise::categorise_page;
pub use claude::{Answer, CallUsage, ClaudeError};
pub use jev::{Jev, JevError, MathPlacement};
pub use transcribe::transcribe_page;

pub(super) use claude::api_key_is_set;
pub(super) use jev::JEV_API_KEY_VARIABLE;

const CLAUDE_RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(30), Duration::from_secs(120)];
const JEV_RETRY_DELAY: Duration = Duration::from_secs(2);
// A short line that is not blank, because a blank one never reaches the API.
const KEY_PROBE_TEXT: &str = "a + b = c";

#[derive(Debug, Clone)]
pub struct PageSource {
    /// 1 for the first page of the chapter.
    pub position: u32,
    pub pdf: PathBuf,
    /// A sharp picture of the page for the models to read. It is deleted when the page is saved.
    pub image: PathBuf,
    /// What `pdftotext -layout` printed for the page.
    pub text_layer: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ServiceError {
    #[error(transparent)]
    Claude(#[from] ClaudeError),

    #[error(transparent)]
    Jev(#[from] JevError),
}

/// The paid calls made for a page. Each one retries inside the implementation, so callers do not.
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

/// The one paid call a picture that stands alone needs. It retries inside the implementation, so
/// callers do not.
pub trait ImageServices {
    /// A transcription of the picture. `correction` says why the last one was rejected.
    fn transcribe(
        &self,
        picture: &Path,
        correction: Option<&str>,
    ) -> impl Future<Output = Result<Answer<TranscribedPage>, ServiceError>> + Send;
}

/// Sonnet through the `claude` command. It needs no Jev key.
pub struct LiveImageServices;

/// Haiku and Sonnet through the `claude` command, and Jev over HTTP.
pub struct LiveServices {
    jev: Jev,
}

impl LiveServices {
    /// Sends Jev one short line, so a rejected key stops the run before any page is started.
    ///
    /// # Errors
    /// - [`ServiceError::Jev`] if there is no key or Jev refuses the key
    pub async fn with_jev_key(jev_api_key: Option<&str>) -> Result<Self, ServiceError> {
        let jev_api_key = jev_api_key.ok_or(JevError::MissingApiKey)?;
        let services = Self {
            jev: Jev::new(jev_api_key)?,
        };
        services.jev_contains_math(KEY_PROBE_TEXT).await?;
        Ok(services)
    }

    async fn jev_contains_math(&self, text: &str) -> Result<Option<MathPlacement>, JevError> {
        match self.jev.contains_math(text).await {
            Err(error) if error.is_worth_retrying() => {
                tokio::time::sleep(JEV_RETRY_DELAY).await;
                self.jev.contains_math(text).await
            }
            outcome => outcome,
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

impl ImageServices for LiveImageServices {
    async fn transcribe(
        &self,
        picture: &Path,
        correction: Option<&str>,
    ) -> Result<Answer<TranscribedPage>, ServiceError> {
        Ok(with_retries(|| transcribe_page(picture, correction)).await?)
    }
}
