//! The tools that read: search the library, write an answer, list the documents, read a page and
//! say whether the services are ready.

use rag_ingestion::health;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};

use super::QuantyServer;
use super::failure::ToolError;
use super::health::HealthResult;
use crate::insights::{self, ChapterInsightsArgs, InsightsView};
use crate::library::{
    self, ChapterConceptsArgs, ChapterContentsArgs, ConceptsView, ContentsView, Documents,
    PageView, ReadPageArgs,
};
use crate::retrieval::{self, AnswerResult, QuestionArgs, SearchArgs, SearchResult};
use crate::services::Services;

#[tool_router(router = reading_router, vis = "pub(super)")]
impl<S: Services> QuantyServer<S> {
    /// Finds the stored items (text chunks, formulas, figures, tables and code listings) that
    /// are nearest to a question, with their document, page and the reason each was found. It
    /// embeds the question with the Gemini API, which is a small paid call. Give the
    /// `document_id` and `page` of a result to `read_page` to read the page around it. Give
    /// `document` (a `title` or `document_id` from `list_documents`) to search one document only.
    #[tool(annotations(read_only_hint = true))]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<Json<SearchResult>, ToolError> {
        let shared = &self.shared;
        Ok(Json(
            retrieval::search(&shared.config, &*shared.services, args).await?,
        ))
    }

    /// Writes an answer to a question from what `search` finds, with the items that each
    /// statement rests on. It uses the subscription of the agent of the Ask choice and embeds the
    /// question with Gemini. It can take a minute or two. An agent that can write its own answer
    /// should call `search` instead.
    #[tool(annotations(read_only_hint = true))]
    async fn answer(
        &self,
        Parameters(args): Parameters<QuestionArgs>,
    ) -> Result<Json<AnswerResult>, ToolError> {
        let shared = &self.shared;
        Ok(Json(
            retrieval::answer(&shared.config, &*shared.services, args).await?,
        ))
    }

    /// Lists every stored document with its media, category, authors, tags and chapter. Give the
    /// `document_id` of one to `read_page`, and its `title` or `document_id` as `document` to
    /// `search` or `answer` to ask that document alone.
    #[tool(annotations(read_only_hint = true))]
    async fn list_documents(&self) -> Result<Json<Documents>, ToolError> {
        Ok(Json(library::list_documents(&self.shared.config).await?))
    }

    /// Reads the pieces of one page of a chapter in reading order, to read around a search result.
    /// It reads the saved chapter and needs no store.
    #[tool(annotations(read_only_hint = true))]
    async fn read_page(
        &self,
        Parameters(args): Parameters<ReadPageArgs>,
    ) -> Result<Json<PageView>, ToolError> {
        Ok(Json(library::read_page(&self.shared.config, args)?))
    }

    /// Lists every formula, figure, table and code listing of one document (a book's chapter, a
    /// part of a book or a paper) in reading order, with the page each is on, its section, label,
    /// caption or name, and its whole text: the LaTeX of a formula, the code as printed, the
    /// explanation of a figure with the path of its picture, the Markdown of a table. A code
    /// listing that page breaks cut is one item. Give `kind` for one kind only. It reads the
    /// saved chapter, needs no store and costs nothing. Give the `page` of an item to `read_page`
    /// to read around it.
    #[tool(annotations(read_only_hint = true))]
    async fn chapter_contents(
        &self,
        Parameters(args): Parameters<ChapterContentsArgs>,
    ) -> Result<Json<ContentsView>, ToolError> {
        Ok(Json(library::chapter_contents(&self.shared.config, args)?))
    }

    /// Lists the concepts that one document discusses, as the ingest found them, the most
    /// mentioned first, each with a one-line definition. Give `page` for the concepts of one page
    /// only, as the desktop app shows beside a page. It reads FalkorDB and costs nothing.
    #[tool(annotations(read_only_hint = true))]
    async fn chapter_concepts(
        &self,
        Parameters(args): Parameters<ChapterConceptsArgs>,
    ) -> Result<Json<ConceptsView>, ToolError> {
        Ok(Json(
            library::chapter_concepts(&self.shared.config, args).await?,
        ))
    }

    /// Gives the insights, takeaways and cheat sheet written for one document when it was
    /// ingested: short Markdown paragraphs with the pages each rests on, one-line points, and a
    /// Markdown sheet of definitions, formulas and rules of thumb. `state` is `saved`,
    /// `not_written_yet` (call `write_insights` with this `document_id`, which is paid) or
    /// `none_for_book_part` (a part of a book that is not a chapter, such as its index, gets
    /// none). It reads the saved file and costs nothing.
    #[tool(annotations(read_only_hint = true))]
    async fn chapter_insights(
        &self,
        Parameters(args): Parameters<ChapterInsightsArgs>,
    ) -> Result<Json<InsightsView>, ToolError> {
        Ok(Json(
            insights::chapter_insights(&self.shared.config, args).await?,
        ))
    }

    /// Says whether Qdrant, FalkorDB, the sign-in of the Ingest choice's agent and the two API
    /// keys are ready, and which model and thinking each kind of work uses. Each key is checked
    /// with the one free call its provider allows; neither Google nor Jev gives a balance to a
    /// key, so a billing page is given instead. It costs nothing.
    #[tool(annotations(read_only_hint = true))]
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
