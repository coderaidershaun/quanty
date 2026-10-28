//! What an agent gives `catalog`, `describe_tool` and `guide`.

use schemars::JsonSchema;
use serde::Deserialize;

use crate::server::meta::Group;

/// What an agent gives `catalog`. Every part is optional.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct CatalogArgs {
    /// One group, to list its tools.
    pub(super) group: Option<Group>,
    /// `summaries` or `names`.
    pub(super) detail: Option<CatalogDetail>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum CatalogDetail {
    /// Each tool with its summary and whether it is paid.
    #[default]
    Summaries,
    /// The names only.
    Names,
}

/// What an agent gives `describe_tool`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct DescribeToolArgs {
    /// A tool name from `catalog`.
    pub(super) name: String,
}

/// What an agent gives `guide`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct GuideArgs {
    /// What the guide is about.
    pub(super) topic: GuideTopic,
}

#[derive(Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GuideTopic {
    Start,
    Research,
    Ingest,
    Jobs,
    Costs,
    Errors,
}

impl GuideTopic {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            GuideTopic::Start => "start",
            GuideTopic::Research => "research",
            GuideTopic::Ingest => "ingest",
            GuideTopic::Jobs => "jobs",
            GuideTopic::Costs => "costs",
            GuideTopic::Errors => "errors",
        }
    }

    /// The Markdown of the guide.
    pub(super) fn text(self) -> &'static str {
        match self {
            GuideTopic::Start => include_str!("guides/start.md"),
            GuideTopic::Research => include_str!("guides/research.md"),
            GuideTopic::Ingest => include_str!("guides/ingest.md"),
            GuideTopic::Jobs => include_str!("guides/jobs.md"),
            GuideTopic::Costs => include_str!("guides/costs.md"),
            GuideTopic::Errors => include_str!("guides/errors.md"),
        }
    }
}
