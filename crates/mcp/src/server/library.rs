//! The tools that browse the library and read it whole: its media, a document's overview and
//! outline, its sections, pages and items, and several reads in one call.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::JsonObject;
use rmcp::{Json, tool, tool_router};

use super::QuantyServer;
use super::failure::ToolError;
use super::meta::{Cost, Group, ToolMarks, output_schema};
use crate::library::{
    self, DocumentOutlineArgs, DocumentOverviewArgs, ItemRead, ListMediaArgs, ManyRead, Media,
    Outline, Overview, PagesView, ReadItemArgs, ReadManyArgs, ReadPagesArgs, ReadSectionArgs,
    SectionView,
};
use crate::services::Services;

/// The first call that a host makes to a tool that lists its results by name.
pub(super) fn names_entry() -> JsonObject {
    JsonObject::from_iter([("detail".to_owned(), "names".into())])
}

#[tool_router(router = library_router, vis = "pub(super)")]
impl<S: Services> QuantyServer<S> {
    /// Media of the library, such as books and papers, with their labels and counts of documents,
    /// pages and items. Give `detail: names` to browse.
    #[tool(
        title = "List media",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<Media>(),
        meta = ToolMarks::new(Group::Library, Cost::Free)
            .with_discovery(names_entry())
            .into()
    )]
    async fn list_media(
        &self,
        Parameters(args): Parameters<ListMediaArgs>,
    ) -> Result<Json<Media>, ToolError> {
        Ok(Json(library::list_media(&self.shared.config, args).await?))
    }

    /// A document at a glance: its insights, takeaways, concepts with mention counts and related
    /// documents. Give `detail: full` for the cheat sheet, outline and every insight.
    #[tool(
        title = "Document overview",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<Overview>(),
        meta = ToolMarks::new(Group::Library, Cost::Free).into()
    )]
    async fn document_overview(
        &self,
        Parameters(args): Parameters<DocumentOverviewArgs>,
    ) -> Result<Json<Overview>, ToolError> {
        Ok(Json(
            library::document_overview(&self.shared.config, args).await?,
        ))
    }

    /// Sections of a document with their headings, page ranges, item counts and concept counts.
    /// Give a section's `index` to `read_section`.
    #[tool(
        title = "Document outline",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<Outline>(),
        meta = ToolMarks::new(Group::Library, Cost::Free)
            .with_discovery_target()
            .into()
    )]
    async fn document_outline(
        &self,
        Parameters(args): Parameters<DocumentOutlineArgs>,
    ) -> Result<Json<Outline>, ToolError> {
        Ok(Json(
            library::document_outline(&self.shared.config, args).await?,
        ))
    }

    /// One section of a document as Markdown or as pieces, with its formulas, figures, tables and
    /// code. A long section continues with `cursor`.
    #[tool(
        title = "Read section",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<SectionView>(),
        meta = ToolMarks::new(Group::Library, Cost::Free).into()
    )]
    async fn read_section(
        &self,
        Parameters(args): Parameters<ReadSectionArgs>,
    ) -> Result<Json<SectionView>, ToolError> {
        Ok(Json(library::read_section(&self.shared.config, args)?))
    }

    /// A run of pages of a document as Markdown or as pieces. A long run continues with `cursor`.
    #[tool(
        title = "Read pages",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<PagesView>(),
        meta = ToolMarks::new(Group::Library, Cost::Free).into()
    )]
    async fn read_pages(
        &self,
        Parameters(args): Parameters<ReadPagesArgs>,
    ) -> Result<Json<PagesView>, ToolError> {
        Ok(Json(library::read_pages(&self.shared.config, args)?))
    }

    /// One formula, figure, table or code listing, by its printed label or its `item_id`, with its
    /// page, section, content and the paragraphs that cite it.
    #[tool(
        title = "Read item",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<ItemRead>(),
        meta = ToolMarks::new(Group::Library, Cost::Free).into()
    )]
    async fn read_item(
        &self,
        Parameters(args): Parameters<ReadItemArgs>,
    ) -> Result<Json<ItemRead>, ToolError> {
        Ok(Json(library::read_item(&self.shared.config, args)?))
    }

    /// Several reads in one call (`read_section`, `read_pages`, `read_item` or `chapter_insights`)
    /// that share one `max_chars` budget.
    #[tool(
        title = "Read many",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<ManyRead>(),
        meta = ToolMarks::new(Group::Research, Cost::Free).into()
    )]
    async fn read_many(
        &self,
        Parameters(args): Parameters<ReadManyArgs>,
    ) -> Result<Json<ManyRead>, ToolError> {
        let wording = |error| ToolError::Insights(error).text();
        Ok(Json(
            library::read_many(&self.shared.config, args, wording).await?,
        ))
    }
}
