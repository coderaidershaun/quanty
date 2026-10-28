//! The tools that change the library or write to the machine: the labels of a media, the tags of a
//! document, and the file an export of insights makes.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};

use super::QuantyServer;
use super::failure::ToolError;
use super::meta::{Cost, Group, ToolMarks, input_schema, output_schema};
use crate::insights::{ExportInsightsArgs, ExportResult, export_insights};
use crate::library::{
    self, DocumentTagsView, EditDocumentTagsArgs, EditMediaArgs, MediaLabelsView,
};
use crate::services::Services;

#[tool_router(router = changes_router, vis = "pub(super)")]
impl<S: Services> QuantyServer<S> {
    /// Change the category, authors or tags of a media, in both stores. Free.
    #[tool(
        title = "Edit media",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true),
        input_schema = input_schema::<EditMediaArgs>(),
        output_schema = output_schema::<MediaLabelsView>(),
        meta = ToolMarks::new(Group::Admin, Cost::Free).into()
    )]
    async fn edit_media(
        &self,
        Parameters(args): Parameters<EditMediaArgs>,
    ) -> Result<Json<MediaLabelsView>, ToolError> {
        Ok(Json(library::edit_media(&self.shared.config, args).await?))
    }

    /// Add or remove tags of one document, in both stores. Free.
    #[tool(
        title = "Edit document tags",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true),
        input_schema = input_schema::<EditDocumentTagsArgs>(),
        output_schema = output_schema::<DocumentTagsView>(),
        meta = ToolMarks::new(Group::Admin, Cost::Free).into()
    )]
    async fn edit_document_tags(
        &self,
        Parameters(args): Parameters<EditDocumentTagsArgs>,
    ) -> Result<Json<DocumentTagsView>, ToolError> {
        Ok(Json(
            library::edit_document_tags(&self.shared.config, args).await?,
        ))
    }

    /// Write a document's saved insights to a PDF, Markdown or HTML file; `overwrite` replaces one. Free.
    #[tool(
        title = "Export insights",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true),
        input_schema = input_schema::<ExportInsightsArgs>(),
        output_schema = output_schema::<ExportResult>(),
        meta = ToolMarks::new(Group::Admin, Cost::Free).into()
    )]
    async fn export_insights(
        &self,
        Parameters(args): Parameters<ExportInsightsArgs>,
    ) -> Result<Json<ExportResult>, ToolError> {
        Ok(Json(export_insights(&self.shared.config, args).await?))
    }
}
