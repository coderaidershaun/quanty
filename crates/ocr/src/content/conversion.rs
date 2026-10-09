//! The record of how a page was made, saved under `conversion` in `page.json`: the tags, the
//! route, the checks and every paid call.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How a page was made. A page written by hand has none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Conversion {
    pub tags: PageCategories,
    pub math_check: MathCheck,
    /// Why the math check gave no answer: the error and its causes, cut short. `None` when it
    /// answered.
    #[serde(default)]
    pub math_check_failure: Option<String>,
    pub route: Route,
    pub route_reasons: Vec<RouteReason>,
    pub checks: Checks,
    pub calls: Vec<CallRecord>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MathCheck {
    Inline,
    Block,
    Both,
    None,
    /// The check itself failed.
    NoAnswer,
}

/// Which model wrote the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    HaikuCopy,
    Sonnet,
    /// Haiku was tried first and its copy was not good enough.
    HaikuThenSonnet,
}

/// Why a page went to Sonnet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteReason {
    TagsReportMath,
    TagsReportFigure,
    TagsReportTable,
    MathCheckReportsMath,
    MathCheckFailed,
    TextLayerAlmostEmpty,
    CopyFlaggedStrongerModel,
    CopyHasNoPieces,
    CopyContainsBackslash,
    CopyFailedReplyCheck,
    CopyFailedCopyCheck,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Checks {
    pub text_layer_words: usize,
    /// Words in the copied kinds only: headings, text, footnotes and tables.
    pub piece_words: usize,
    pub word_match: WordMatch,
    /// The page was reported to hold displayed math but has no formula piece.
    pub displayed_math_without_formula: bool,
    /// A second reply was asked for because the first one broke a rule.
    pub reply_retried: bool,
    /// What was wrong with the first reply, in the sentence the model was told. `None` when no
    /// second reply was asked for.
    #[serde(default)]
    pub retry_reason: Option<String>,
    /// The figures whose picture is the whole page, because their own could not be cut.
    #[serde(default)]
    pub whole_page_figures: Vec<WholePageFigure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WholePageFigure {
    /// The figure's piece number on its page.
    pub piece: u32,
    pub why: String,
}

/// How much of the copied words and of the text layer's words match, from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WordMatch {
    pub piece_words_in_text_layer: f64,
    pub text_layer_words_in_pieces: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CallRecord {
    pub step: CallStep,
    pub model: String,
    pub cost_usd: f64,
    /// A file saved before the input and cache counts were kept reads them as 0.
    #[serde(default)]
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
    pub thinking_tokens: u64,
    pub seconds: f64,
}

/// Which paid call a [`CallRecord`] is for. The math check is not one of them: it reports no
/// cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CallStep {
    Tag,
    Copy,
    Transcribe,
}
