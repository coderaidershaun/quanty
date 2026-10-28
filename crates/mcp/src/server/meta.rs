//! What a tool says about itself beyond its arguments: the marks in its `_meta` that hosts read,
//! and the schema of its result.

use std::sync::Arc;

use rmcp::handler::server::tool::schema_for_output;
use rmcp::model::{JsonObject, MetaObject};
use schemars::JsonSchema;
use serde_json::{Value, json};

/// The `_meta` key of the group that a tool belongs to.
pub(crate) const GROUP_KEY: &str = "quanty/group";

/// The `_meta` key that says whether a tool is paid, and by whom.
pub(crate) const COST_KEY: &str = "quanty/cost";

/// The `_meta` key that marks a tool as an entry point for a host that discovers tools by calling
/// them.
pub(crate) const DISCOVERY_KEY: &str = "jet/discovery";

/// The `_meta` key that marks a tool as one that starts a job, and names the tool that reports on
/// it.
pub(crate) const JOB_KEY: &str = "jet/job";

/// The version that the `jet/discovery` and `jet/job` marks both carry.
const JET_MARK_VERSION: u32 = 1;

/// The group of a tool. Keep the variants in the order that the groups should take in the tool
/// list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[expect(dead_code, reason = "most groups have no tool that names them yet")]
pub(crate) enum Group {
    Catalog,
    Library,
    Research,
    Concepts,
    Jobs,
    Ingest,
    Datasets,
    Admin,
}

impl Group {
    fn as_str(self) -> &'static str {
        match self {
            Group::Catalog => "catalog",
            Group::Library => "library",
            Group::Research => "research",
            Group::Concepts => "concepts",
            Group::Jobs => "jobs",
            Group::Ingest => "ingest",
            Group::Datasets => "datasets",
            Group::Admin => "admin",
        }
    }
}

/// Whether a tool spends money, and whose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cost {
    Free,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "no paid tool declares its cost yet")
    )]
    Paid(Payer),
}

impl Cost {
    fn to_json(self) -> Value {
        match self {
            Cost::Free => json!({ "paid": false }),
            Cost::Paid(payer) => json!({ "paid": true, "who": payer.as_str() }),
        }
    }
}

/// Whose usage or credit a paid tool spends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(dead_code, reason = "no paid tool names its payer yet")]
pub(crate) enum Payer {
    Ask,
    Ingest,
    Datasets,
    Gemini,
}

impl Payer {
    fn as_str(self) -> &'static str {
        match self {
            Payer::Ask => "ask",
            Payer::Ingest => "ingest",
            Payer::Datasets => "datasets",
            Payer::Gemini => "gemini",
        }
    }
}

/// The `_meta` of one tool. Every tool has a group and a cost; the `jet/` marks are optional.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolMarks(JsonObject);

impl ToolMarks {
    pub(crate) fn new(group: Group, cost: Cost) -> ToolMarks {
        let mut marks = JsonObject::new();
        marks.insert(GROUP_KEY.to_owned(), group.as_str().into());
        marks.insert(COST_KEY.to_owned(), cost.to_json());
        ToolMarks(marks)
    }

    /// Marks the tool as an entry point, with the arguments that a host should call it with first.
    #[cfg_attr(not(test), expect(dead_code, reason = "no tool is an entry point yet"))]
    pub(crate) fn with_discovery(mut self, entry_arguments: JsonObject) -> ToolMarks {
        let mark = json!({ "version": JET_MARK_VERSION, "entryArguments": entry_arguments });
        self.0.insert(DISCOVERY_KEY.to_owned(), mark);
        self
    }

    /// Marks the tool as one that starts a job: `status_tool` reports on it, and `id_field` is the
    /// field of the result that holds the id to give to `status_tool`.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "no tool is marked as starting a job yet")
    )]
    pub(crate) fn with_job(mut self, status_tool: &str, id_field: &str) -> ToolMarks {
        let mark = json!({
            "version": JET_MARK_VERSION,
            "statusTool": status_tool,
            "idField": id_field,
        });
        self.0.insert(JOB_KEY.to_owned(), mark);
        self
    }
}

impl From<ToolMarks> for MetaObject {
    fn from(marks: ToolMarks) -> MetaObject {
        MetaObject(marks.0)
    }
}

/// The schema of the result of a tool whose result is `T`. A tool declares its schema with this
/// instead of leaving it to the tool macro, so that one place controls the text of every schema.
pub(crate) fn output_schema<T: JsonSchema + 'static>() -> Arc<JsonObject> {
    schema_for_output::<T>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_have_the_wire_shape_that_hosts_read() {
        let entry_arguments = json!({ "group": "library" });
        let marks = ToolMarks::new(Group::Library, Cost::Paid(Payer::Gemini))
            .with_discovery(entry_arguments.as_object().unwrap().clone())
            .with_job("job_status", "job_id");

        let meta = MetaObject::from(marks);

        assert_eq!(
            Value::Object(meta.0),
            json!({
                "quanty/group": "library",
                "quanty/cost": { "paid": true, "who": "gemini" },
                "jet/discovery": { "version": 1, "entryArguments": { "group": "library" } },
                "jet/job": { "version": 1, "statusTool": "job_status", "idField": "job_id" },
            })
        );
    }
}
