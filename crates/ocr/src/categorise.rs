//! Finds out what kinds of content a PDF page holds, so `ocr` can choose how to handle it.

use std::path::Path;
use std::time::Duration;

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::{Deserialize, Serialize};

use crate::claude::{self, Answer, ClaudeCall, ClaudeError};

// An alias for the newest Haiku model, so answers can change when a new one comes out.
const MODEL: &str = "haiku";
const TAGGING_TIMEOUT: Duration = Duration::from_secs(120);

const SYSTEM_PROMPT: &str = "You categorise a single page from a PDF book. Read the page you are given, then report which kinds of content are visibly present on it. Mark a category true only when that content appears on the page itself, and mark every other category false.";

/// Which kinds of content are present on one PDF page.
///
/// The JSON schema the model must follow is generated from this struct, so the two cannot
/// disagree. Each `description` is the wording the model reads for that category, so changing
/// one changes the answers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(description = "Which kinds of content are present on one PDF page.")]
pub struct PageCategories {
    #[schemars(
        description = "Displayed mathematical notation: an equation or formula set on its own line or lines, apart from the running text."
    )]
    pub math_notation: bool,
    #[schemars(
        description = "Mathematical notation inside a line of running text, such as symbols, subscripts or short expressions within a sentence. Plain numbers, money amounts, percentages and dates are not mathematical notation."
    )]
    pub inline_with_text_math_notation: bool,
    #[schemars(
        description = "A chapter number printed on the page, whether where a chapter opens or in a running header or footer."
    )]
    pub chapter_number: bool,
    #[schemars(
        description = "A chapter title printed on the page, whether where a chapter opens or in a running header or footer."
    )]
    pub chapter_name: bool,
    #[schemars(description = "A printed page number in the header or footer.")]
    pub page_number: bool,
    #[schemars(
        description = "A two-dimensional chart with more than one y-axis or more than one x-axis, each with its own scale, such as a second axis on the right-hand side."
    )]
    pub diagram_2d_multi_axis_chart: bool,
    #[schemars(
        description = "A two-dimensional line, scatter or area chart drawn against exactly one x-axis and one y-axis."
    )]
    pub diagram_2d_single_axis_chart: bool,
    #[schemars(description = "A three-dimensional surface plot drawn against three axes.")]
    pub diagram_3d_surface_chart: bool,
    #[schemars(description = "A two-dimensional bar chart or histogram.")]
    pub diagram_2d_bar_chart: bool,
    #[schemars(
        description = "A single two-dimensional chart that combines different kinds of marks, such as bars with a line drawn over them."
    )]
    pub diagram_2d_mixed_chart: bool,
    #[schemars(
        description = "Any other diagram that is not one of the chart kinds above, such as a flowchart, tree, network or schematic."
    )]
    pub diagram_other: bool,
    #[schemars(description = "A photograph or picture that is not a chart or diagram.")]
    pub image: bool,
    #[schemars(description = "Data laid out in rows and columns.")]
    pub table: bool,
    #[schemars(description = "A section or sub-section heading below chapter level.")]
    pub sub_heading: bool,
}

/// Asks Claude which kinds of content are on a single-page PDF.
///
/// This runs the `claude` command line tool, which must be installed and logged in.
///
/// The file must hold exactly one page. A longer PDF is read whole and categorised without
/// complaint, so the answer would describe several pages at once.
///
/// The call gives up after 120 seconds and does not retry. It refuses to run when
/// `ANTHROPIC_API_KEY` is set, because `claude` would then bill the API instead of the
/// subscription.
///
/// # Errors
/// - [`ClaudeError::ApiKeySet`] if `ANTHROPIC_API_KEY` is set
/// - [`ClaudeError::FileUnreadable`] if `pdf_page` does not exist or cannot be resolved
/// - [`ClaudeError::Spawn`] if the `claude` command cannot be started, or waiting for it to
///   finish fails
/// - [`ClaudeError::TimedOut`] if `claude` has not answered after 120 seconds
/// - [`ClaudeError::Exited`] if `claude` fails and prints nothing readable
/// - [`ClaudeError::UnreadableResponse`] if `claude` succeeds but prints something other than
///   its JSON result
/// - [`ClaudeError::RunFailed`] if the run finished without producing categories
/// - [`ClaudeError::UnexpectedReply`] if the categories do not fit [`PageCategories`]
pub async fn categorise_page(pdf_page: &Path) -> Result<PageCategories, ClaudeError> {
    Ok(categorise_page_with_usage(pdf_page).await?.value)
}

/// Like [`categorise_page`], and also reports what the call used.
pub(crate) async fn categorise_page_with_usage(
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
