//! The `read_many` tool: several reads in one call, which share one budget of characters.

use rag_core::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::items::{ReadItemArgs, read_item};
use super::sections::{ReadPagesArgs, ReadSectionArgs, read_pages, read_section};
use crate::args::{Truncation, checked_max_chars};
use crate::insights::{ChapterInsightsArgs, InsightsToolError, chapter_insights};
use crate::library::LibraryError;

/// The most requests one call may hold.
const MOST_REQUESTS: usize = 20;

/// What an agent gives `read_many`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ReadManyArgs {
    /// From 1 to 20 reads, done in order.
    requests: Vec<ReadRequest>,
    /// At most this many characters of text over all the reads. 24000 when left out.
    max_chars: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
struct ReadRequest {
    tool: ReadTool,
    /// The arguments of that tool. Its own `max_chars` can only make the read smaller.
    args: Value,
}

#[derive(Deserialize, Serialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ReadTool {
    ReadSection,
    ReadPages,
    ReadItem,
    ChapterInsights,
}

impl ReadTool {
    fn name(self) -> &'static str {
        match self {
            ReadTool::ReadSection => "read_section",
            ReadTool::ReadPages => "read_pages",
            ReadTool::ReadItem => "read_item",
            ReadTool::ChapterInsights => "chapter_insights",
        }
    }
}

/// The results of the reads of one call, in the order they were asked for.
#[derive(Serialize, JsonSchema)]
pub(crate) struct ManyRead {
    results: Vec<RequestResult>,
    /// The characters of text over all the results.
    used_chars: usize,
}

#[derive(Serialize, JsonSchema)]
struct RequestResult {
    tool: ReadTool,
    /// True when the result stops short, or when no characters were left to read it at all.
    truncated: bool,
    /// What the tool gives. Left out when the read failed or was not made.
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    /// Why the read failed, in the words of a failed call of that tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// What a request gave: its result as JSON and how it stands against its budget.
struct Read {
    result: Value,
    truncated: bool,
    used: usize,
}

/// Does each read in order until the characters run out. A read that fails does not fail the call:
/// it is an `error` in its place, worded by `wording` as a failed call of the tool would be.
///
/// # Errors
/// A number of requests that is 0 or over 20, and a `max_chars` that is refused.
pub(crate) async fn read_many(
    config: &Config,
    args: ReadManyArgs,
    wording: impl Fn(InsightsToolError) -> String,
) -> Result<ManyRead, LibraryError> {
    let given = args.requests.len();
    if !(1..=MOST_REQUESTS).contains(&given) {
        return Err(LibraryError::RequestCount {
            given,
            most: MOST_REQUESTS,
        });
    }
    let mut left = checked_max_chars(args.max_chars)?;
    let mut used_chars = 0;
    let mut results = Vec::with_capacity(given);
    for request in args.requests {
        let tool = request.tool;
        if left == 0 {
            results.push(RequestResult {
                tool,
                truncated: true,
                result: None,
                error: None,
            });
            continue;
        }
        results.push(match read_one(config, request, left, &wording).await {
            Ok(read) => {
                used_chars += read.used;
                left = left.saturating_sub(read.used);
                RequestResult {
                    tool,
                    truncated: read.truncated,
                    result: Some(read.result),
                    error: None,
                }
            }
            Err(error) => RequestResult {
                tool,
                truncated: false,
                result: None,
                error: Some(error),
            },
        });
    }
    Ok(ManyRead {
        results,
        used_chars,
    })
}

/// One request, read with at most `left` characters, or the words of why it failed.
async fn read_one(
    config: &Config,
    request: ReadRequest,
    left: usize,
    wording: &impl Fn(InsightsToolError) -> String,
) -> Result<Read, String> {
    let tool = request.tool;
    let not_valid = |error: serde_json::Error| {
        format!(
            "the `args` of the `{}` request are not valid: {error}; give the arguments that `{}` takes",
            tool.name(),
            tool.name()
        )
    };
    let fitted = |own: Option<u32>| Some(own.map_or(left, |own| (own as usize).min(left)) as u32);
    let value = request.args;
    match tool {
        ReadTool::ReadSection => {
            let mut args: ReadSectionArgs = serde_json::from_value(value).map_err(not_valid)?;
            args.max_chars = fitted(args.max_chars);
            let view = read_section(config, args).map_err(|error| wording(error.into()))?;
            finish(&view, view.truncation())
        }
        ReadTool::ReadPages => {
            let mut args: ReadPagesArgs = serde_json::from_value(value).map_err(not_valid)?;
            args.max_chars = fitted(args.max_chars);
            let view = read_pages(config, args).map_err(|error| wording(error.into()))?;
            finish(&view, view.truncation())
        }
        ReadTool::ReadItem => {
            let mut args: ReadItemArgs = serde_json::from_value(value).map_err(not_valid)?;
            args.max_chars = fitted(args.max_chars);
            let view = read_item(config, args).map_err(|error| wording(error.into()))?;
            finish(&view, view.truncation())
        }
        ReadTool::ChapterInsights => {
            let mut args: ChapterInsightsArgs = serde_json::from_value(value).map_err(not_valid)?;
            args.max_chars = fitted(args.max_chars);
            let view = chapter_insights(config, args).await.map_err(wording)?;
            finish(&view, view.truncation())
        }
    }
}

fn finish(view: &impl Serialize, truncation: &Truncation) -> Result<Read, String> {
    let result = serde_json::to_value(view)
        .map_err(|error| format!("the result could not be written as JSON: {error}"))?;
    Ok(Read {
        result,
        truncated: truncation.is_truncated(),
        used: truncation.used(),
    })
}
