//! The tools that read: search the library, write an answer, list the documents, read a page and
//! say whether the services are ready.

use rag_ingestion::health;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};

use super::QuantyServer;
use super::failure::ToolError;
use super::health::HealthResult;
use super::library::names_entry;
use super::meta::{Cost, Group, Payer, ToolMarks, input_schema, output_schema};
use crate::insights::{self, ChapterInsightsArgs, InsightsView};
use crate::library::{
    self, ChapterConceptsArgs, ChapterContentsArgs, ConceptsView, ContentsView, Documents,
    ListDocumentsArgs, PageView, ReadPageArgs,
};
use crate::retrieval::{self, AnswerResult, QuestionArgs, SearchArgs, SearchResult};
use crate::services::Services;

#[tool_router(router = reading_router, vis = "pub(super)")]
impl<S: Services> QuantyServer<S> {
    /// Search, PAID (one Gemini embedding): the stored items nearest a question, with document, page and why.
    #[tool(
        title = "Search",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<SearchArgs>(),
        output_schema = output_schema::<SearchResult>(),
        meta = ToolMarks::new(Group::Research, Cost::Paid(Payer::Gemini)).into()
    )]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<Json<SearchResult>, ToolError> {
        let shared = &self.shared;
        Ok(Json(
            retrieval::search(&shared.config, &*shared.services, args).await?,
        ))
    }

    /// Answer, PAID (Ask agent subscription, one Gemini embedding): a written answer citing items. Slow.
    #[tool(
        title = "Answer",
        annotations(read_only_hint = true),
        input_schema = input_schema::<QuestionArgs>(),
        output_schema = output_schema::<AnswerResult>(),
        meta = ToolMarks::new(Group::Research, Cost::Paid(Payer::Ask)).into()
    )]
    async fn answer(
        &self,
        Parameters(args): Parameters<QuestionArgs>,
    ) -> Result<Json<AnswerResult>, ToolError> {
        let shared = &self.shared;
        Ok(Json(
            retrieval::answer(&shared.config, &*shared.services, args).await?,
        ))
    }

    /// Documents of the library with media, category, authors, tags and chapter. Filter and page.
    #[tool(
        title = "List documents",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<ListDocumentsArgs>(),
        output_schema = output_schema::<Documents>(),
        meta = ToolMarks::new(Group::Library, Cost::Free)
            .with_discovery(names_entry())
            .into()
    )]
    async fn list_documents(
        &self,
        Parameters(args): Parameters<ListDocumentsArgs>,
    ) -> Result<Json<Documents>, ToolError> {
        Ok(Json(
            library::list_documents(&self.shared.config, args).await?,
        ))
    }

    /// One page of a chapter in reading order, to read around a search result.
    #[tool(
        title = "Read page",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<ReadPageArgs>(),
        output_schema = output_schema::<PageView>(),
        meta = ToolMarks::new(Group::Library, Cost::Free).into()
    )]
    async fn read_page(
        &self,
        Parameters(args): Parameters<ReadPageArgs>,
    ) -> Result<Json<PageView>, ToolError> {
        Ok(Json(library::read_page(&self.shared.config, args)?))
    }

    /// Formulas, figures, tables and code listings of a document in reading order, with full text.
    #[tool(
        title = "Chapter contents",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<ChapterContentsArgs>(),
        output_schema = output_schema::<ContentsView>(),
        meta = ToolMarks::new(Group::Library, Cost::Free).into()
    )]
    async fn chapter_contents(
        &self,
        Parameters(args): Parameters<ChapterContentsArgs>,
    ) -> Result<Json<ContentsView>, ToolError> {
        Ok(Json(library::chapter_contents(&self.shared.config, args)?))
    }

    /// Concepts of a document, most mentioned first, with one-line definitions. `page` for one page.
    #[tool(
        title = "Chapter concepts",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<ChapterConceptsArgs>(),
        output_schema = output_schema::<ConceptsView>(),
        meta = ToolMarks::new(Group::Concepts, Cost::Free).into()
    )]
    async fn chapter_concepts(
        &self,
        Parameters(args): Parameters<ChapterConceptsArgs>,
    ) -> Result<Json<ConceptsView>, ToolError> {
        Ok(Json(
            library::chapter_concepts(&self.shared.config, args).await?,
        ))
    }

    /// Saved insights, takeaways and cheat sheet of a document.
    #[tool(
        title = "Chapter insights",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<ChapterInsightsArgs>(),
        output_schema = output_schema::<InsightsView>(),
        meta = ToolMarks::new(Group::Library, Cost::Free).into()
    )]
    async fn chapter_insights(
        &self,
        Parameters(args): Parameters<ChapterInsightsArgs>,
    ) -> Result<Json<InsightsView>, ToolError> {
        Ok(Json(
            insights::chapter_insights(&self.shared.config, args).await?,
        ))
    }

    /// Readiness of Qdrant, FalkorDB, the agent sign-in and the API keys, and the model choices. Free.
    #[tool(
        title = "Health",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<HealthResult>(),
        meta = ToolMarks::new(Group::Admin, Cost::Free).into()
    )]
    async fn health(&self) -> Json<HealthResult> {
        let shared = &self.shared;
        let (services, keys) = tokio::join!(
            health::check(&shared.config),
            shared.services.check_keys(&shared.config),
        );
        Json(HealthResult::new(&services, keys, |work| {
            shared.services.model_choice(work)
        }))
    }
}
