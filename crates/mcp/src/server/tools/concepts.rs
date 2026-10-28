//! The tools that explore the concept graph: find concepts, read one, follow its relations and
//! items, compare documents, and find items like an item.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::JsonObject;
use rmcp::{Json, tool, tool_router};

use crate::concepts::{
    self, CompareDocumentsArgs, CompareResult, ConceptArgs, ConceptItemsArgs, ConceptItemsResult,
    ConceptNeighboursArgs, ConceptPathArgs, ConceptView, DocumentConceptsArgs,
    DocumentConceptsResult, FindConceptsArgs, FindConceptsResult, NeighboursResult, PathResult,
    SimilarItemsArgs, SimilarItemsResult,
};
use crate::server::QuantyServer;
use crate::server::failure::ToolError;
use crate::server::meta::{Cost, Group, Payer, ToolMarks, input_schema, output_schema};
use crate::services::Services;

#[tool_router(router = concepts_router, vis = "pub(in crate::server)")]
impl<S: Services> QuantyServer<S> {
    /// Concepts, PAID (one Gemini embedding) for `query`; `name` is free. With aliases and counts.
    #[tool(
        title = "Find concepts",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<FindConceptsArgs>(),
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

    /// One concept: definition, aliases, mentions, documents, first items. Nodes lead to its items and neighbours.
    #[tool(
        title = "Concept",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<ConceptArgs>(),
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

    /// Relations of a concept one or two steps out, each with the item that stated it.
    #[tool(
        title = "Concept neighbours",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<ConceptNeighboursArgs>(),
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

    /// Items that mention a concept, paged, with document and page. Read them with `read_item` or `read_pages`.
    #[tool(
        title = "Concept items",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<ConceptItemsArgs>(),
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

    /// Shortest chain of relations between two concepts, each with its stating item.
    #[tool(
        title = "Concept path",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<ConceptPathArgs>(),
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

    /// Concepts shared by 2 to 10 documents, and those only one has, with mention counts.
    #[tool(
        title = "Compare documents",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<CompareDocumentsArgs>(),
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

    /// Concepts of a document or one page, with ids, a `limit` and optional mention counts.
    #[tool(
        title = "Document concepts",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<DocumentConceptsArgs>(),
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

    /// Stored items nearest to an item by its vector. Free: it embeds nothing.
    #[tool(
        title = "Similar items",
        annotations(read_only_hint = true, idempotent_hint = true),
        input_schema = input_schema::<SimilarItemsArgs>(),
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
