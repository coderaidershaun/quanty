//! The connection to FalkorDB and the check that the store behind it answers. The statements
//! that write are in `writes`, and the statements that read are in `reads`, `records` and `edges`.

mod edges;
mod reads;
mod records;
mod writes;

use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use falkordb::{
    AsyncGraph, FalkorAsyncClient, FalkorClientBuilder, FalkorConnectionInfo, FalkorDBError,
    FalkorValue, QueryResult, RowStream,
};
use rag_core::{ConceptId, Config, DocId, ItemId};

use crate::contents::{
    BookNode, ConceptAlias, ConceptNode, DocumentNode, DocumentRecord, ItemMentions, ItemNode,
    Mention, Relation,
};
use crate::store::{GraphError, GraphStore};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

/// An open connection to FalkorDB, and the graph that the config names.
pub struct FalkorGraph {
    client: FalkorAsyncClient,
    graph: AsyncGraph,
    url: String,
}

impl FalkorGraph {
    /// Opens the connection, so a store that is down fails here. The client needs the
    /// multi-thread runtime of tokio.
    ///
    /// # Errors
    /// [`GraphError::Connect`] when the address is not valid or nothing answers there.
    pub async fn connect(config: &Config) -> Result<FalkorGraph, GraphError> {
        let url = config.falkordb_url.clone();
        let connect_error = |source| GraphError::Connect {
            url: config.falkordb_url.clone(),
            source,
        };
        let info = FalkorConnectionInfo::try_from(url.as_str()).map_err(connect_error)?;
        let building = FalkorClientBuilder::new_async()
            .with_connection_info(info)
            .with_response_timeout(Some(RESPONSE_TIMEOUT))
            .build();
        // Every future of the FalkorDB client is boxed where it is made. They are so deeply
        // nested that, left inside the future of a caller, they make the compiler stop with
        // "queries overflow the depth limit".
        let client = Box::pin(building).await.map_err(connect_error)?;
        let graph = client.select_graph(&config.falkordb_graph);
        Ok(FalkorGraph { client, graph, url })
    }

    /// Asks the store for its list of graphs, which is the cheapest request that needs the
    /// store to be working.
    ///
    /// # Errors
    /// - [`GraphError::Ping`] when the store does not answer
    /// - [`GraphError::NotFalkorDb`] when something answers that is not FalkorDB
    pub async fn ping(&self) -> Result<(), GraphError> {
        let url = self.url.clone();
        // Boxed for the same reason as the future in `connect`.
        Box::pin(self.client.list_graphs())
            .await
            .map(|_| ())
            .map_err(|source| match source {
                FalkorDBError::ParsingArray => GraphError::NotFalkorDb { url, source },
                source => GraphError::Ping { url, source },
            })
    }

    /// The only place that waits for a query. It boxes the future for the same reason as in
    /// `connect`.
    pub(crate) async fn run(
        &self,
        action: &'static str,
        statement: &str,
        parameters: Vec<(&'static str, FalkorValue)>,
    ) -> Result<QueryResult<RowStream>, GraphError> {
        // Each query needs the graph handle by `&mut`, and a clone is cheap.
        let mut graph = self.graph.clone();
        let query = graph.query(statement).with_params(parameters).execute();
        // The boxed future also gets a type that hides what is inside it. Otherwise, to prove that
        // a caller's future can move to another thread, the compiler walks every future nested in
        // the client, and it stops with an overflow error on a caller that spawns a whole ingest.
        let query: Pin<Box<dyn Future<Output = _> + Send + '_>> = Box::pin(query);
        query.await.map_err(|source| GraphError::Query {
            url: self.url.clone(),
            graph: self.graph.graph_name().to_owned(),
            action,
            source,
        })
    }
}

impl GraphStore for FalkorGraph {
    async fn upsert_document(&self, document: &DocumentNode) -> Result<(), GraphError> {
        writes::upsert_document(self, document).await
    }

    async fn documents(&self) -> Result<Vec<DocumentNode>, GraphError> {
        reads::documents(self).await
    }

    async fn document_records(&self) -> Result<Vec<DocumentRecord>, GraphError> {
        records::document_records(self).await
    }

    async fn add_book(&self, book: &BookNode) -> Result<(), GraphError> {
        writes::add_book(self, book).await
    }

    async fn books(&self) -> Result<Vec<BookNode>, GraphError> {
        reads::books(self).await
    }

    async fn upsert_items(&self, document: DocId, items: &[ItemNode]) -> Result<(), GraphError> {
        writes::upsert_items(self, document, items).await
    }

    async fn set_ingested_items(
        &self,
        document: DocId,
        items: Option<u64>,
    ) -> Result<(), GraphError> {
        writes::set_ingested_items(self, document, items).await
    }

    async fn ingested_items(&self, document: DocId) -> Result<Option<u64>, GraphError> {
        reads::ingested_items(self, document).await
    }

    async fn set_chapter_folder(&self, document: DocId, folder: &Path) -> Result<(), GraphError> {
        writes::set_chapter_folder(self, document, folder).await
    }

    async fn delete_document(&self, id: DocId) -> Result<u64, GraphError> {
        writes::delete_document(self, id).await
    }

    async fn upsert_concept(&self, concept: &ConceptNode) -> Result<(), GraphError> {
        writes::upsert_concept(self, concept).await
    }

    async fn add_mentions(&self, mentions: &[Mention]) -> Result<(), GraphError> {
        writes::add_mentions(self, mentions).await
    }

    async fn add_relations(&self, relations: &[Relation]) -> Result<(), GraphError> {
        writes::add_relations(self, relations).await
    }

    async fn add_alias(&self, alias: &ConceptAlias) -> Result<(), GraphError> {
        writes::add_alias(self, alias).await
    }

    async fn find_concept_by_name(
        &self,
        normalised_name: &str,
    ) -> Result<Option<ConceptNode>, GraphError> {
        reads::find_concept_by_name(self, normalised_name).await
    }

    async fn concept(&self, id: ConceptId) -> Result<Option<ConceptNode>, GraphError> {
        reads::concept(self, id).await
    }

    async fn concepts_for_items(&self, items: &[ItemId]) -> Result<Vec<ConceptNode>, GraphError> {
        reads::concepts_for_items(self, items).await
    }

    async fn items_for_concepts(
        &self,
        concepts: &[ConceptId],
        limit: usize,
    ) -> Result<Vec<ItemMentions>, GraphError> {
        reads::items_for_concepts(self, concepts, limit).await
    }

    async fn related_concepts(
        &self,
        concepts: &[ConceptId],
    ) -> Result<Vec<ConceptNode>, GraphError> {
        reads::related_concepts(self, concepts).await
    }

    async fn concepts_on_page(
        &self,
        document: DocId,
        page: u32,
    ) -> Result<Vec<ConceptNode>, GraphError> {
        reads::concepts_on_page(self, document, page).await
    }

    async fn relations_among(&self, concepts: &[ConceptId]) -> Result<Vec<Relation>, GraphError> {
        edges::relations_among(self, concepts).await
    }

    async fn mentions_between(
        &self,
        items: &[ItemId],
        concepts: &[ConceptId],
    ) -> Result<Vec<Mention>, GraphError> {
        edges::mentions_between(self, items, concepts).await
    }
}

/// An id as the text that Qdrant also uses for it.
fn id_value(id: impl ToString) -> FalkorValue {
    FalkorValue::String(id.to_string())
}
