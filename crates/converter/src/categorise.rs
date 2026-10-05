//! Finds out what kinds of content a PDF page holds, so the converter can choose how to handle it.

use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output, Stdio};

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

// An alias for the newest Haiku model, so answers can change when a new one comes out.
const MODEL: &str = "haiku";

const SYSTEM_PROMPT: &str = "You categorise a single page from a PDF book. Read the page you are given, then report which kinds of content are visibly present on it. Mark a category true only when that content appears on the page itself, and mark every other category false.";

/// Which kinds of content are present on one PDF page.
///
/// The JSON schema the model must follow is generated from this struct, so the two cannot
/// disagree. Each `description` is the wording the model reads for that category, so changing
/// one changes the answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(description = "Which kinds of content are present on one PDF page.")]
pub struct PageCategories {
    #[schemars(
        description = "Displayed mathematical notation: an equation or formula set on its own line or lines, apart from the running text."
    )]
    pub math_notation: bool,
    #[schemars(
        description = "Mathematical notation inside a line of running text, such as symbols, subscripts or short expressions within a sentence."
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

/// Why a page could not be categorised.
#[derive(thiserror::Error, Debug)]
pub enum CategoriseError {
    #[error("pdf page is missing or unreadable at {}", path.display())]
    PageUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not start the claude command; check that it is installed and on PATH")]
    Spawn(#[source] std::io::Error),

    #[error("claude failed ({status}) with no readable response, stderr {stderr:?}")]
    Exited {
        status: ExitStatus,
        stderr: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("claude response was not the expected json")]
    UnreadableResponse(#[source] serde_json::Error),

    #[error(
        "claude run ended with subtype {subtype} and no categories: {}",
        join_reasons(.result.as_deref(), .errors)
    )]
    NoCategories {
        subtype: String,
        result: Option<String>,
        errors: Vec<String>,
    },
}

// SMELL: an empty `result` string counts as a reason, so the message ends with a bare colon.
fn join_reasons(result: Option<&str>, errors: &[String]) -> String {
    let reasons: Vec<&str> = result
        .into_iter()
        .chain(errors.iter().map(String::as_str))
        .collect();
    if reasons.is_empty() {
        "no reason given".to_owned()
    } else {
        reasons.join("; ")
    }
}

// Do not add `deny_unknown_fields`: `claude` adds new fields to this object over time.
#[derive(Deserialize)]
struct HeadlessResult {
    subtype: String,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    structured_output: Option<PageCategories>,
}

/// Asks Claude which kinds of content are on a single-page PDF.
///
/// This runs the `claude` command line tool, which must be installed and logged in.
///
/// The file must hold exactly one page. A longer PDF is read whole and categorised without
/// complaint, so the answer would describe several pages at once.
///
/// There is no timeout or retry. Wrap the call in `tokio::time::timeout` to bound it; dropping
/// the future kills the `claude` process.
///
/// # Errors
/// - [`CategoriseError::PageUnreadable`] if `pdf_page` does not exist or cannot be resolved
/// - [`CategoriseError::Spawn`] if the `claude` command cannot be started, or waiting for it to
///   finish fails
/// - [`CategoriseError::Exited`] if `claude` fails and prints nothing readable
/// - [`CategoriseError::UnreadableResponse`] if `claude` succeeds but prints something other than
///   its JSON result
/// - [`CategoriseError::NoCategories`] if the run finished without producing categories
pub async fn categorise_page(pdf_page: &Path) -> Result<PageCategories, CategoriseError> {
    let absolute_path =
        std::fs::canonicalize(pdf_page).map_err(|source| CategoriseError::PageUnreadable {
            path: pdf_page.to_path_buf(),
            source,
        })?;
    let prompt = format!(
        "Categorise the PDF page at {}. Read the whole file with the Read tool first, with no page range.",
        absolute_path.display()
    );

    // The prompt must come right after `-p`: `--tools` and `--allowedTools` take any number of
    // values and would swallow a prompt placed after them.
    let output = Command::new("claude")
        .arg("-p")
        .arg(prompt)
        .args(["--model", MODEL])
        .args(["--output-format", "json"])
        .arg("--json-schema")
        .arg(page_categories_schema().to_string())
        .args(["--system-prompt", SYSTEM_PROMPT])
        // Do not remove: without this flag the run loads the instructions, hooks and plugins of
        // whatever project it is started in, and a hook could then fire for every page.
        .arg("--safe-mode")
        // Do not widen: Read is the only tool the model is given and anything else is refused, so
        // text on a page cannot make it write files, run commands or fetch anything.
        .args(["--tools", "Read"])
        .args(["--allowedTools", "Read"])
        .args(["--permission-mode", "dontAsk"])
        .arg("--no-session-persistence")
        // Without this, `claude` waits a few seconds for input on the caller's stdin.
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        // SMELL: a failure while waiting for `claude` is also reported as a failure to start it.
        .map_err(CategoriseError::Spawn)?;

    read_response(&output)
}

fn read_response(output: &Output) -> Result<PageCategories, CategoriseError> {
    // Stdout is read before the exit status: a run that fails still prints a JSON result saying
    // why, and exits with a failure status.
    match serde_json::from_slice::<HeadlessResult>(&output.stdout) {
        Ok(HeadlessResult {
            subtype,
            structured_output: Some(categories),
            ..
        }) if subtype == "success" => Ok(categories),
        Ok(run) => Err(CategoriseError::NoCategories {
            subtype: run.subtype,
            result: run.result,
            errors: run.errors,
        }),
        Err(source) if output.status.success() => Err(CategoriseError::UnreadableResponse(source)),
        Err(source) => Err(CategoriseError::Exited {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            source,
        }),
    }
}

// `claude` rejects schemas newer than draft 07, and schemars defaults to a newer one.
fn page_categories_schema() -> serde_json::Value {
    SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<PageCategories>()
        .to_value()
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
