//! The tools that tell an agent what the other tools are: an index of them by group, one tool in
//! detail, and short guides. The long notes of every tool live here as Markdown files.

mod args;
mod doc;
mod error;
mod table;
mod view;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::JsonObject;
use rmcp::{Json, tool, tool_router};

pub(crate) use args::{CatalogArgs, DescribeToolArgs, GuideArgs};
pub(super) use error::CatalogError;
pub(super) use table::in_list_order;
use view::{CatalogResult, GuideText, ToolDescription};

use super::QuantyServer;
use super::failure::ToolError;
use super::meta::{Cost, Group, ToolMarks, input_schema, output_schema};
use crate::services::Services;

#[tool_router(router = catalog_router, vis = "pub(super)")]
impl<S: Services> QuantyServer<S> {
    /// Index of the tools by group, with a summary of each. Start here.
    #[tool(
        title = "Catalog",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<CatalogArgs>(),
        output_schema = output_schema::<CatalogResult>(),
        meta = ToolMarks::new(Group::Catalog, Cost::Free)
            .with_discovery(JsonObject::new())
            .into()
    )]
    async fn catalog(
        &self,
        Parameters(args): Parameters<CatalogArgs>,
    ) -> Result<Json<CatalogResult>, ToolError> {
        let tools = in_list_order(self.tool_router.list_all());
        Ok(Json(CatalogResult::of(&tools, &args)))
    }

    /// One tool in detail: long description, argument notes, examples, errors, cost, related tools.
    #[tool(
        title = "Describe tool",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<DescribeToolArgs>(),
        output_schema = output_schema::<ToolDescription>(),
        meta = ToolMarks::new(Group::Catalog, Cost::Free).into()
    )]
    async fn describe_tool(
        &self,
        Parameters(args): Parameters<DescribeToolArgs>,
    ) -> Result<Json<ToolDescription>, ToolError> {
        let tools = self.tool_router.list_all();
        Ok(Json(ToolDescription::of(&tools, &args.name)?))
    }

    /// A short guide: start, research, ingest, jobs, costs or errors.
    #[tool(
        title = "Guide",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<GuideArgs>(),
        output_schema = output_schema::<GuideText>(),
        meta = ToolMarks::new(Group::Catalog, Cost::Free).into()
    )]
    async fn guide(
        &self,
        Parameters(args): Parameters<GuideArgs>,
    ) -> Result<Json<GuideText>, ToolError> {
        Ok(Json(GuideText::of(args.topic)))
    }
}
