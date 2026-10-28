//! The one list of the tools, in the order the tool list shows them inside a group, with the notes
//! of each tool.

use rmcp::model::Tool;

use crate::server::meta::{COST_KEY, GROUP_KEY, Group};

/// A tool and the Markdown notes that `describe_tool` serves for it.
pub(super) struct Entry {
    pub(super) name: &'static str,
    pub(super) notes: &'static str,
}

macro_rules! entries {
    ($($name:literal),* $(,)?) => {
        [$(Entry { name: $name, notes: include_str!(concat!($name, ".md")) }),*]
    };
}

/// Every tool, grouped as `Group` orders the groups. A tool that is missing here sorts last in the
/// tool list.
pub(super) const TOOLS: [Entry; 40] = entries![
    "catalog",
    "describe_tool",
    "guide",
    "list_media",
    "list_documents",
    "document_overview",
    "document_outline",
    "read_section",
    "read_pages",
    "read_item",
    "read_page",
    "chapter_contents",
    "chapter_insights",
    "search",
    "read_many",
    "answer",
    "find_concepts",
    "concept",
    "concept_neighbours",
    "concept_items",
    "concept_path",
    "compare_documents",
    "document_concepts",
    "chapter_concepts",
    "similar_items",
    "job_status",
    "watch_job",
    "list_jobs",
    "stop_job",
    "ingest_status",
    "insights_status",
    "dataset_status",
    "check_pdf",
    "ingest_pdf",
    "write_insights",
    "build_dataset",
    "health",
    "edit_media",
    "edit_document_tags",
    "export_insights",
];

/// The notes of the tool with this name.
pub(super) fn notes_of(name: &str) -> Option<&'static str> {
    TOOLS
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.notes)
}

/// The group a tool marks itself with.
pub(super) fn group_of(tool: &Tool) -> Option<Group> {
    let marks = tool.meta.as_ref()?;
    Group::parse(marks.0.get(GROUP_KEY)?.as_str()?)
}

/// Whether a tool marks itself as paid.
pub(super) fn is_paid(tool: &Tool) -> bool {
    tool.meta
        .as_ref()
        .and_then(|marks| marks.0.get(COST_KEY))
        .and_then(|cost| cost.get("paid"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// The tools in the order of the tool list: by group, then by place in [`TOOLS`].
pub(crate) fn in_list_order(mut tools: Vec<Tool>) -> Vec<Tool> {
    tools.sort_by_key(|tool| {
        let group = group_of(tool).map_or(Group::ALL.len(), |group| group as usize);
        let place = TOOLS
            .iter()
            .position(|entry| entry.name == tool.name)
            .unwrap_or(TOOLS.len());
        (group, place)
    });
    tools
}
