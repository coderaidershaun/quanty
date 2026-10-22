//! The tools that change the library: the labels of a media and the tags of a document.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};

use super::QuantyServer;
use super::failure::ToolError;
use crate::library::{
    self, DocumentTagsView, EditDocumentTagsArgs, EditMediaArgs, MediaLabelsView,
};
use crate::services::Services;

#[tool_router(router = changes_router, vis = "pub(super)")]
impl<S: Services> QuantyServer<S> {
    /// Changes the labels of a stored media: its category, its authors or its tags. A label that
    /// is given replaces the old one and one that is left out stays; `[]` takes every author or
    /// every tag away. Every document of the media, and every stored item of those documents,
    /// gets the new labels in both stores, so `search` and `answer` filter by them at once. It
    /// embeds nothing, asks no model and costs nothing. The title names the media and cannot
    /// change. The category says how the next PDF of the media must be named.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = true
    ))]
    async fn edit_media(
        &self,
        Parameters(args): Parameters<EditMediaArgs>,
    ) -> Result<Json<MediaLabelsView>, ToolError> {
        Ok(Json(library::edit_media(&self.shared.config, args).await?))
    }

    /// Adds tags to one stored document, or takes tags away, in both stores. The tags of its
    /// media stay: change those with `edit_media`. It embeds nothing, asks no model and costs
    /// nothing. Adding a tag the document has, or taking away one it lacks, changes nothing.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = true
    ))]
    async fn edit_document_tags(
        &self,
        Parameters(args): Parameters<EditDocumentTagsArgs>,
    ) -> Result<Json<DocumentTagsView>, ToolError> {
        Ok(Json(
            library::edit_document_tags(&self.shared.config, args).await?,
        ))
    }
}
