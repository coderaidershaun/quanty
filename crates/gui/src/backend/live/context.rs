//! The settings and the outside services that every kind of work shares, and the one place that
//! turns a backend error into something a person can read.

use std::sync::Arc;

use graph::{FalkorGraph, GraphStore};
use ocr::ConvertError;
use ocr::convert::services::{LiveServices, PageServices};
use rag_core::{ClaudeCli, ConceptStore, Config, Embedder, GeminiEmbedder, ItemStore, Llm};
use rag_ingestion::{ConceptExtractor, EXTRACTION_MODEL, Models, Stores};
use rag_retrieval::{ANSWER_MODEL, Retriever};

use crate::contract::Failure;

/// Makes the outside services. `RealServices` makes the real ones; a test makes stand-ins.
pub trait Services: Send + Sync + 'static {
    type Embedder: Embedder + Send + Sync + 'static;
    type Llm: Llm + Send + Sync + 'static;
    type Graph: GraphStore + Send + Sync + 'static;
    /// The paid calls that a page needs. A test hands out stand-ins here, so a test of an ingest
    /// can never reach the paid converter.
    type Pages: PageServices + Send + Sync + 'static;

    fn embedder(&self, config: &Config) -> Result<Self::Embedder, Failure>;
    fn llm(&self, model: &str) -> Self::Llm;
    fn graph(&self, config: &Config) -> impl Future<Output = Result<Self::Graph, Failure>> + Send;
    fn page_services(
        &self,
        jev_api_key: Option<&str>,
    ) -> impl Future<Output = Result<Arc<Self::Pages>, ConvertError>> + Send;
}

/// The real Gemini embedder, `claude` command, FalkorDB graph, and page converter.
pub struct RealServices;

impl Services for RealServices {
    type Embedder = GeminiEmbedder;
    type Llm = ClaudeCli;
    type Graph = FalkorGraph;
    type Pages = LiveServices;

    fn embedder(&self, config: &Config) -> Result<GeminiEmbedder, Failure> {
        Ok(GeminiEmbedder::from_config(config)?)
    }

    fn llm(&self, model: &str) -> ClaudeCli {
        ClaudeCli::new(model)
    }

    async fn graph(&self, config: &Config) -> Result<FalkorGraph, Failure> {
        Ok(FalkorGraph::connect(config).await?)
    }

    async fn page_services(
        &self,
        jev_api_key: Option<&str>,
    ) -> Result<Arc<LiveServices>, ConvertError> {
        Ok(Arc::new(LiveServices::with_jev_key(jev_api_key).await?))
    }
}

/// The retriever that a context keeps once the stores answered. Without this name clippy's
/// `type_complexity` fails the field that holds it.
pub type Kept<S> = Arc<Retriever<<S as Services>::Embedder, <S as Services>::Graph>>;

pub struct LiveContext<S: Services> {
    config: Config,
    services: S,
}

impl<S: Services> LiveContext<S> {
    /// Opens nothing. Every connection is made when a command needs it.
    pub fn new(config: Config, services: S) -> Self {
        LiveContext { config, services }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// A retriever over the stores and the embedder.
    ///
    /// # Errors
    /// A failure that says which store or key is not ready.
    pub async fn retriever(&self) -> Result<Kept<S>, Failure> {
        Ok(Arc::new(Retriever {
            embedder: self.services.embedder(&self.config)?,
            items: ItemStore::connect(&self.config).map_err(|error| self.failure(error))?,
            concepts: ConceptStore::connect(&self.config).map_err(|error| self.failure(error))?,
            graph: self.services.graph(&self.config).await?,
        }))
    }

    /// The graph alone: a read that needs no embedder, such as the concepts of a page, must not
    /// fail for a missing Gemini key.
    ///
    /// # Errors
    /// A failure that says the graph store is not ready.
    pub async fn graph(&self) -> Result<Arc<S::Graph>, Failure> {
        Ok(Arc::new(self.services.graph(&self.config).await?))
    }

    /// Fresh stores, for one writing job.
    ///
    /// # Errors
    /// A failure that says which store is not ready.
    pub async fn stores(&self) -> Result<Stores<S::Graph>, Failure> {
        Ok(Stores {
            items: ItemStore::connect(&self.config).map_err(|error| self.failure(error))?,
            graph: self.services.graph(&self.config).await?,
            concepts: ConceptStore::connect(&self.config).map_err(|error| self.failure(error))?,
        })
    }

    /// The page services, opened with the Jev key of the settings, so no other code sees the
    /// key. Ask for them only when a page is left to convert: the real ones call Jev once.
    ///
    /// # Errors
    /// Whatever stops the page services from opening, such as a missing or refused key.
    pub async fn page_services(&self) -> Result<Arc<S::Pages>, ConvertError> {
        let key = self.config.jev_api_key.as_ref().map(|key| key.expose());
        self.services.page_services(key).await
    }

    /// The models of an ingest.
    ///
    /// # Errors
    /// A failure that says the embedding key is missing.
    pub fn models(&self) -> Result<Models<S::Embedder, S::Llm>, Failure> {
        Ok(Models {
            embedder: self.services.embedder(&self.config)?,
            concepts: ConceptExtractor::new(self.services.llm(EXTRACTION_MODEL), &self.config),
        })
    }

    pub fn answer_llm(&self) -> S::Llm {
        self.services.llm(ANSWER_MODEL)
    }

    /// Turns a backend error into a `Failure`. Every kind of work converts its errors through it.
    pub fn failure(&self, error: impl Into<Failure>) -> Failure {
        error.into()
    }
}
