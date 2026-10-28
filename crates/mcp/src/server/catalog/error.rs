//! Why `describe_tool` could not describe a tool.

#[derive(thiserror::Error, Debug)]
pub(crate) enum CatalogError {
    #[error("no tool is named {name:?}; call `catalog` to see the tools and their names")]
    UnknownTool { name: String },

    #[error("an example in the notes of `{tool}` is not a JSON object of arguments")]
    BadExample {
        tool: String,
        #[source]
        source: serde_json::Error,
    },
}
