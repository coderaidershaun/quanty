//! Finds out what kinds of content a PDF page holds, so `ocr` can choose how to handle it.

use std::path::Path;
use std::time::Duration;

use schemars::generate::SchemaSettings;

use super::claude::{self, Answer, ClaudeCall, ClaudeError};
use crate::content::PageCategories;

/// The model that tags each page with the kinds of content on it.
// An alias for the newest Haiku model, so answers can change when a new one comes out.
pub const TAG_MODEL: &str = "haiku";
const TAGGING_TIMEOUT: Duration = Duration::from_secs(120);

const SYSTEM_PROMPT: &str = "You categorise a single page from a PDF book. Read the page you are given, then report which kinds of content are visibly present on it. Mark a category true only when that content appears on the page itself, and mark every other category false.";

/// Asks Claude which kinds of content are on a single-page PDF, and reports what the call used.
///
/// The file must hold exactly one page. A longer PDF is read whole and categorised without
/// complaint, so the answer would describe several pages at once.
pub(super) async fn categorise_page_with_usage(
    pdf_page: &Path,
) -> Result<Answer<PageCategories>, ClaudeError> {
    claude::run(&ClaudeCall {
        model: TAG_MODEL,
        effort: None,
        system_prompt: SYSTEM_PROMPT,
        instruction: "Categorise the PDF page",
        correction: None,
        schema: &page_categories_schema(),
        file: pdf_page,
        also_read: None,
        timeout: TAGGING_TIMEOUT,
    })
    .await
}

// `claude` rejects schemas newer than draft 07, and schemars defaults to a newer one.
fn page_categories_schema() -> serde_json::Value {
    let mut schema = SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<PageCategories>()
        .to_value();
    // Do not remove: the answers were tuned with the keys in sorted order, and the schema crate
    // now keeps declaration order instead.
    schema.sort_all_objects();
    schema
}
