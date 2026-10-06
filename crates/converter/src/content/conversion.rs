//! The record of how a page was made, saved under `conversion` in `page.json`: the tags, the
//! route, the checks and every paid call.

use serde::{Deserialize, Serialize};

use crate::categorise::PageCategories;

/// How a page was made. A page written by hand has none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Conversion {
    pub tags: PageCategories,
    pub math_check: MathCheck,
    pub route: Route,
    pub route_reasons: Vec<RouteReason>,
    pub checks: Checks,
    pub calls: Vec<CallRecord>,
}

/// What the math check said about the page's text.
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

/// What was measured on the page that was saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Checks {
    pub text_layer_words: usize,
    /// Words in the copied kinds only: headings, text, footnotes and tables.
    pub piece_words: usize,
    pub word_match: WordMatch,
    /// The page was reported to hold displayed math but has no formula piece.
    pub displayed_math_without_formula: bool,
    /// The first Sonnet reply failed the reply check and the second was saved.
    pub reply_retried: bool,
}

/// How much of the copied words and of the text layer's words match, from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WordMatch {
    pub piece_words_in_text_layer: f64,
    pub text_layer_words_in_pieces: f64,
}

/// One paid model call made for a page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CallRecord {
    pub step: CallStep,
    pub model: String,
    pub cost_usd: f64,
    pub output_tokens: u64,
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
