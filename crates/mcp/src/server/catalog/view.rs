//! What `catalog`, `describe_tool` and `guide` give back.

use rmcp::model::{JsonObject, Tool};
use schemars::JsonSchema;
use serde::Serialize;

use super::args::{CatalogArgs, CatalogDetail, GuideTopic};
use super::doc::Notes;
use super::error::CatalogError;
use super::table::{group_of, is_paid, notes_of};
use crate::discovery::{Discovery, Node};
use crate::server::meta::Group;

/// The tools that an agent should call first, in order, and why.
const START_HERE: [(&str, &str); 5] = [
    ("health", "Check that the stores and the keys are ready."),
    (
        "list_documents",
        "Find the document you want, with `detail: names`; keep its `document_id`.",
    ),
    (
        "document_overview",
        "See the document at a glance: insights, takeaways, concepts and sections.",
    ),
    ("read_section", "Read one section of it in full."),
    (
        "search",
        "Find passages for a question in the whole library.",
    ),
];

#[derive(Serialize, JsonSchema)]
pub(crate) struct CatalogResult {
    #[serde(flatten)]
    discovery: Discovery,
    groups: Vec<GroupView>,
    start_here: Vec<StartStep>,
}

#[derive(Serialize, JsonSchema)]
struct GroupView {
    name: &'static str,
    about: &'static str,
    tools: Vec<ToolView>,
}

#[derive(Serialize, JsonSchema)]
struct ToolView {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    paid: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
struct StartStep {
    tool: &'static str,
    why: &'static str,
}

impl CatalogResult {
    /// The catalog of `tools`, which are in list order: all groups as nodes that open them, or one
    /// group as nodes that call its tools.
    pub(super) fn of(tools: &[Tool], args: &CatalogArgs) -> CatalogResult {
        let detail = args.detail.unwrap_or_default();
        let wanted = args
            .group
            .map_or_else(|| Group::ALL.to_vec(), |group| vec![group]);
        let groups: Vec<GroupView> = wanted
            .iter()
            .map(|group| GroupView::of(*group, tools, detail))
            .collect();
        let nodes = match args.group {
            None => wanted.iter().map(|group| group_node(*group)).collect(),
            Some(group) => tools_in(group, tools).map(tool_node).collect(),
        };
        CatalogResult {
            discovery: Discovery::of(nodes),
            groups,
            start_here: START_HERE
                .iter()
                .map(|(tool, why)| StartStep { tool, why })
                .collect(),
        }
    }
}

impl GroupView {
    fn of(group: Group, tools: &[Tool], detail: CatalogDetail) -> GroupView {
        let tools = tools_in(group, tools)
            .map(|tool| match detail {
                CatalogDetail::Names => ToolView {
                    name: tool.name.to_string(),
                    summary: None,
                    paid: None,
                },
                CatalogDetail::Summaries => ToolView {
                    name: tool.name.to_string(),
                    summary: Some(tool.description.as_deref().unwrap_or_default().to_owned()),
                    paid: Some(is_paid(tool)),
                },
            })
            .collect();
        GroupView {
            name: group.as_str(),
            about: group.about(),
            tools,
        }
    }
}

fn tools_in(group: Group, tools: &[Tool]) -> impl Iterator<Item = &Tool> {
    tools
        .iter()
        .filter(move |tool| group_of(tool) == Some(group))
}

fn group_node(group: Group) -> Node {
    let arguments = JsonObject::from_iter([("group".to_owned(), group.as_str().into())]);
    Node::discovering(group.as_str(), "catalog", arguments)
}

/// A node that calls a tool with the first example of its notes, or with no arguments.
fn tool_node(tool: &Tool) -> Node {
    let suggested = notes_of(&tool.name)
        .and_then(|text| Notes::parse(&tool.name, text).examples().ok())
        .and_then(|examples| examples.into_iter().next())
        .unwrap_or_default();
    let label = tool.title.as_deref().unwrap_or(&tool.name);
    Node::reading(label, &tool.name, suggested)
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct ToolDescription {
    name: String,
    title: String,
    group: &'static str,
    paid: bool,
    cost: String,
    description: String,
    arguments: String,
    examples: Vec<JsonObject>,
    errors: Vec<String>,
    related: Vec<String>,
}

impl ToolDescription {
    /// # Errors
    /// [`CatalogError::UnknownTool`] when no listed tool has the name.
    pub(super) fn of(tools: &[Tool], name: &str) -> Result<ToolDescription, CatalogError> {
        let unknown = || CatalogError::UnknownTool {
            name: name.to_owned(),
        };
        let name = name.trim();
        let tool = tools
            .iter()
            .find(|tool| tool.name == name)
            .ok_or_else(unknown)?;
        let text = notes_of(name).ok_or_else(unknown)?;
        let notes = Notes::parse(name, text);
        Ok(ToolDescription {
            name: name.to_owned(),
            title: tool.title.clone().unwrap_or_default(),
            group: group_of(tool).map_or("", Group::as_str),
            paid: is_paid(tool),
            cost: notes.section("Cost"),
            description: notes.description().to_owned(),
            arguments: notes.section("Arguments"),
            examples: notes.examples()?,
            errors: notes.bullets("Errors"),
            related: notes.related(),
        })
    }
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct GuideText {
    topic: &'static str,
    text: &'static str,
}

impl GuideText {
    pub(super) fn of(topic: GuideTopic) -> GuideText {
        GuideText {
            topic: topic.as_str(),
            text: topic.text(),
        }
    }
}
