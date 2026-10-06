//! The two calls that ask for a page's reply: a plain copy by Haiku, or a full transcription by
//! Sonnet.

use std::path::Path;
use std::time::Duration;

use super::claude::{self, Answer, ClaudeCall, ClaudeError};
use super::schema::reply_schema;
use crate::convert::reply::{CopiedPage, TranscribedPage};

const COPY_MODEL: &str = "haiku";
const COPY_INSTRUCTION: &str = "Copy the text of the page in the file";
const COPY_PROMPT: &str = include_str!("prompts/copy.md");

// Pinned: the prompts were tuned against this model.
const TRANSCRIBE_MODEL: &str = "claude-sonnet-5-5";
const TRANSCRIBE_EFFORT: &str = "medium";
const TRANSCRIBE_INSTRUCTION: &str = "Transcribe the page in the file";
const TRANSCRIBE_PROMPT: &str = include_str!("prompts/transcribe.md");

const REPLY_TIMEOUT: Duration = Duration::from_secs(180);

/// Asks Haiku to copy the text of a page, or to say the page needs the stronger model. One
/// attempt, no retry.
pub(super) async fn copy_page(page_file: &Path) -> Result<Answer<CopiedPage>, ClaudeError> {
    claude::run(&ClaudeCall {
        model: COPY_MODEL,
        effort: None,
        system_prompt: COPY_PROMPT,
        instruction: COPY_INSTRUCTION,
        correction: None,
        schema: &reply_schema::<CopiedPage>(),
        file: page_file,
        also_read: None,
        timeout: REPLY_TIMEOUT,
    })
    .await
}

/// Asks Sonnet to break the page in `page_file` into its pieces. One attempt, no retry.
///
/// `page_file` is a one-page PDF or an image of a page. `correction` says why the last reply was
/// rejected, for a second try; it has no closing full stop.
pub async fn transcribe_page(
    page_file: &Path,
    correction: Option<&str>,
) -> Result<Answer<TranscribedPage>, ClaudeError> {
    transcribe_page_with_picture(page_file, None, correction).await
}

/// Like [`transcribe_page`], and the model also reads `picture`, a sharper image of the same
/// page. The PDF's text layer anchors letters and indices; the picture shows bold weight and
/// small subscripts. `picture` must sit in the same folder as `page_file`.
pub(super) async fn transcribe_page_with_picture(
    page_file: &Path,
    picture: Option<&Path>,
    correction: Option<&str>,
) -> Result<Answer<TranscribedPage>, ClaudeError> {
    claude::run(&ClaudeCall {
        model: TRANSCRIBE_MODEL,
        effort: Some(TRANSCRIBE_EFFORT),
        system_prompt: TRANSCRIBE_PROMPT,
        instruction: TRANSCRIBE_INSTRUCTION,
        correction,
        schema: &reply_schema::<TranscribedPage>(),
        file: page_file,
        also_read: picture,
        timeout: REPLY_TIMEOUT,
    })
    .await
}
