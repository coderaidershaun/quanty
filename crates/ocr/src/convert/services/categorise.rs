//! Finds out what kinds of content a PDF page holds, so `ocr` can choose how to handle it.

use std::path::Path;
use std::time::Duration;

use schemars::generate::SchemaSettings;

use super::claude::{self, Answer, ClaudeCall, ClaudeError};
use crate::content::PageCategories;

// An alias for the newest Haiku model, so answers can change when a new one comes out.
const MODEL: &str = "haiku";
const TAGGING_TIMEOUT: Duration = Duration::from_secs(120);

const SYSTEM_PROMPT: &str = "You categorise a single page from a PDF book. Read the page you are given, then report which kinds of content are visibly present on it. Mark a category true only when that content appears on the page itself, and mark every other category false.";

/// Asks Claude which kinds of content are on a single-page PDF.
///
/// This runs the `claude` command line tool, which must be installed and logged in.
///
/// The file must hold exactly one page. A longer PDF is read whole and categorised without
/// complaint, so the answer would describe several pages at once.
///
/// It does not retry, and it refuses to run when `ANTHROPIC_API_KEY` is set, because `claude`
/// would then bill the API instead of the subscription.
pub async fn categorise_page(pdf_page: &Path) -> Result<PageCategories, ClaudeError> {
    Ok(categorise_page_with_usage(pdf_page).await?.value)
}

/// Like [`categorise_page`], and also reports what the call used.
pub(super) async fn categorise_page_with_usage(
    pdf_page: &Path,
) -> Result<Answer<PageCategories>, ClaudeError> {
    claude::run(&ClaudeCall {
        model: MODEL,
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn schema_is_draft_07_and_names_every_category() {
        let schema = page_categories_schema();
        assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
        assert_eq!(schema["additionalProperties"], false);
        let required: BTreeSet<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap())
            .collect();
        let expected = BTreeSet::from([
            "math-notation",
            "inline-with-text-math-notation",
            "chapter-number",
            "chapter-name",
            "page-number",
            "diagram-2d-multi-axis-chart",
            "diagram-2d-single-axis-chart",
            "diagram-3d-surface-chart",
            "diagram-2d-bar-chart",
            "diagram-2d-mixed-chart",
            "diagram-other",
            "image",
            "table",
            "sub-heading",
        ]);
        assert_eq!(required, expected);
    }
}
