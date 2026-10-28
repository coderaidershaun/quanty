//! The tools that explore the concept graph: find concepts, read one, follow its relations and
//! items, compare documents, and find items like an item.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::JsonObject;
use rmcp::{Json, tool, tool_router};

use super::QuantyServer;
use super::failure::ToolError;
use super::meta::{Cost, Group, Payer, ToolMarks, output_schema};
use crate::concepts::{
    self, CompareDocumentsArgs, CompareResult, ConceptArgs, ConceptItemsArgs, ConceptItemsResult,
    ConceptNeighboursArgs, ConceptPathArgs, ConceptView, DocumentConceptsArgs,
    DocumentConceptsResult, FindConceptsArgs, FindConceptsResult, NeighboursResult, PathResult,
    SimilarItemsArgs, SimilarItemsResult,
};
use crate::services::Services;

#[tool_router(router = concepts_router, vis = "pub(super)")]
impl<S: Services> QuantyServer<S> {
    /// Concept search: `query` is PAID (one Gemini embedding call) and finds concepts by meaning,
    /// `name` is free and finds one by its exact name or alias. Give exactly one. Each concept
    /// comes with its aliases, its mention count and its document count.
    #[tool(
        title = "Find concepts",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<FindConceptsResult>(),
        meta = ToolMarks::new(Group::Concepts, Cost::Paid(Payer::Gemini)).into()
    )]
    async fn find_concepts(
        &self,
        Parameters(args): Parameters<FindConceptsArgs>,
    ) -> Result<Json<FindConceptsResult>, ToolError> {
        let shared = &self.shared;
        Ok(Json(
            concepts::find_concepts(&shared.config, &*shared.services, args).await?,
        ))
    }

    /// Concept card: definition, aliases, mention count, documents and first items, with nodes
    /// that lead to its items and to the concepts next to it. Give `concept_id` or `name`; with
    /// neither, the nodes lead to the most mentioned concepts. Free.
    #[tool(
        title = "Concept",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<ConceptView>(),
        meta = ToolMarks::new(Group::Concepts, Cost::Free)
            .with_discovery(JsonObject::new())
            .into()
    )]
    async fn concept(
        &self,
        Parameters(args): Parameters<ConceptArgs>,
    ) -> Result<Json<ConceptView>, ToolError> {
        let reference = args.reference()?;
        Ok(Json(
            concepts::concept(&self.shared.config, reference).await?,
        ))
    }

    /// Concept neighbours: the relations (`DERIVED_FROM`, `ASSUMES`, `GENERALISES`, `PART_OF`,
    /// `USED_FOR`) that touch a concept, one or two steps out, each with the item that stated it.
    /// Free.
    #[tool(
        title = "Concept neighbours",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<NeighboursResult>(),
        meta = ToolMarks::new(Group::Concepts, Cost::Free).into()
    )]
    async fn concept_neighbours(
        &self,
        Parameters(args): Parameters<ConceptNeighboursArgs>,
    ) -> Result<Json<NeighboursResult>, ToolError> {
        Ok(Json(
            concepts::concept_neighbours(&self.shared.config, args).await?,
        ))
    }

    /// Concept items: the items that mention a concept, a page at a time, each with its document
    /// and page. Give a row's `document_id` and `page` to `read_page`. Free.
    #[tool(
        title = "Concept items",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<ConceptItemsResult>(),
        meta = ToolMarks::new(Group::Concepts, Cost::Free).into()
    )]
    async fn concept_items(
        &self,
        Parameters(args): Parameters<ConceptItemsArgs>,
    ) -> Result<Json<ConceptItemsResult>, ToolError> {
        Ok(Json(
            concepts::concept_items(&self.shared.config, args).await?,
        ))
    }

    /// Concept path: the shortest chain of relations between two concepts, given by id or name,
    /// followed either way, each relation with the item that stated it. Free.
    #[tool(
        title = "Concept path",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<PathResult>(),
        meta = ToolMarks::new(Group::Concepts, Cost::Free).into()
    )]
    async fn concept_path(
        &self,
        Parameters(args): Parameters<ConceptPathArgs>,
    ) -> Result<Json<PathResult>, ToolError> {
        Ok(Json(
            concepts::concept_path(&self.shared.config, args).await?,
        ))
    }

    /// Compare documents: the concepts that 2 to 10 documents share, and those that only one has,
    /// with how many items of each document mention them. Free.
    #[tool(
        title = "Compare documents",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<CompareResult>(),
        meta = ToolMarks::new(Group::Concepts, Cost::Free).into()
    )]
    async fn compare_documents(
        &self,
        Parameters(args): Parameters<CompareDocumentsArgs>,
    ) -> Result<Json<CompareResult>, ToolError> {
        Ok(Json(
            concepts::compare_documents(&self.shared.config, args).await?,
        ))
    }

    /// Document concepts: the concepts of a document, or of one page, as `chapter_concepts` gives
    /// them, with concept ids, a `limit` and optional mention counts. Free.
    #[tool(
        title = "Document concepts",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<DocumentConceptsResult>(),
        meta = ToolMarks::new(Group::Concepts, Cost::Free).into()
    )]
    async fn document_concepts(
        &self,
        Parameters(args): Parameters<DocumentConceptsArgs>,
    ) -> Result<Json<DocumentConceptsResult>, ToolError> {
        Ok(Json(
            concepts::document_concepts(&self.shared.config, args).await?,
        ))
    }

    /// Similar items: the stored items nearest to a stored item by its vector, nearest first.
    /// Free: it reads Qdrant only and embeds nothing.
    #[tool(
        title = "Similar items",
        annotations(read_only_hint = true, idempotent_hint = true),
        output_schema = output_schema::<SimilarItemsResult>(),
        meta = ToolMarks::new(Group::Concepts, Cost::Free).into()
    )]
    async fn similar_items(
        &self,
        Parameters(args): Parameters<SimilarItemsArgs>,
    ) -> Result<Json<SimilarItemsResult>, ToolError> {
        Ok(Json(
            concepts::similar_items(&self.shared.config, args).await?,
        ))
    }
}
