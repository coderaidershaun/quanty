//! The settings and the outside services that every kind of work shares, and the one place that
//! turns a backend error into something a person can read.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use graph::FalkorGraph;
use ocr::convert::services::{Jev, JevError};
use ocr::{ChapterJob, ConversionSummary, ConvertError, PageProgress};
use rag_core::{
    AgentCli, ApiKey, ConceptStore, Config, EmbedError, Embedder, GeminiEmbedder, ItemStore, Llm,
    WorkKind,
};
use rag_ingestion::{ConceptExtractor, Models, Stores};
use rag_retrieval::Retriever;

use crate::contract::{Failure, FailureKind};

/// A store that neither answers nor refuses must not hold a command for ever.
const CONNECT_LIMIT: Duration = Duration::from_secs(5);

/// `RealServices` makes the real ones; a test makes stand-ins.
pub trait Services: Send + Sync + 'static {
    type Embedder: Embedder + Send + Sync + 'static;
    type Llm: Llm + Send + Sync + 'static;

    fn embedder(&self, config: &Config) -> Result<Self::Embedder, Failure>;
    fn llm(&self, work: WorkKind) -> Self::Llm;
    fn graph(&self, config: &Config) -> impl Future<Output = Result<FalkorGraph, Failure>> + Send;

    /// Converts the pages of the chapter that are not converted yet, and tells each page to
    /// `on_page` as it is cut out and as it ends. This is the part of an ingest that pays for a
    /// model call for every page.
    ///
    /// # Errors
    /// The errors of `ocr::convert_chapter_with_jev_key`.
    fn convert(
        &self,
        job: &ChapterJob,
        jev_api_key: Option<&str>,
        on_page: &mut (dyn FnMut(PageProgress) + Send),
    ) -> impl Future<Output = Result<ConversionSummary, ConvertError>> + Send;

    /// Asks Google whether it accepts `key`. It costs nothing, and it cannot see spent credit.
    fn check_gemini_key(&self, key: &ApiKey)
    -> impl Future<Output = Result<(), EmbedError>> + Send;

    /// Asks Jev whether it accepts `key`. It costs nothing, and Jev documents no reply to it for
    /// spent credit.
    fn check_jev_key(&self, key: &ApiKey) -> impl Future<Output = Result<(), JevError>> + Send;
}

pub struct RealServices;

impl Services for RealServices {
    type Embedder = GeminiEmbedder;
    type Llm = AgentCli;

    fn embedder(&self, config: &Config) -> Result<GeminiEmbedder, Failure> {
        Ok(GeminiEmbedder::from_config(config)?)
    }

    fn llm(&self, work: WorkKind) -> AgentCli {
        AgentCli::for_work(work)
    }

    async fn graph(&self, config: &Config) -> Result<FalkorGraph, Failure> {
        Ok(FalkorGraph::connect(config).await?)
    }

    async fn convert(
        &self,
        job: &ChapterJob,
        jev_api_key: Option<&str>,
        on_page: &mut (dyn FnMut(PageProgress) + Send),
    ) -> Result<ConversionSummary, ConvertError> {
        ocr::convert_chapter_with_jev_key(job, jev_api_key, on_page).await
    }

    async fn check_gemini_key(&self, key: &ApiKey) -> Result<(), EmbedError> {
        GeminiEmbedder::new(key)?.check_key().await
    }

    async fn check_jev_key(&self, key: &ApiKey) -> Result<(), JevError> {
        Jev::new(key.expose())?.check_key().await
    }
}

/// The retriever that a context keeps once the stores answered. Without this name clippy's
/// `type_complexity` fails the field that holds it.
pub type Kept<S> = Arc<Retriever<<S as Services>::Embedder>>;

pub struct LiveContext<S: Services> {
    config: Config,
    services: S,
    retriever: tokio::sync::Mutex<Option<(u64, Kept<S>)>>,
    graph: tokio::sync::Mutex<Option<(u64, Arc<FalkorGraph>)>>,
    /// Goes up when a store fails. What was kept in an earlier era is dropped at its next use,
    /// so a store that comes back is connected again with nothing restarted.
    era: AtomicU64,
}

impl<S: Services> LiveContext<S> {
    pub fn new(config: Config, services: S) -> Self {
        LiveContext {
            config,
            services,
            retriever: tokio::sync::Mutex::new(None),
            graph: tokio::sync::Mutex::new(None),
            era: AtomicU64::new(0),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// # Errors
    /// A failure that says which store or key is not ready.
    pub async fn retriever(&self) -> Result<Kept<S>, Failure> {
        let mut kept = self.retriever.lock().await;
        let era = self.era.load(Ordering::Acquire);
        if let Some((built, retriever)) = kept.as_ref()
            && *built == era
        {
            return Ok(Arc::clone(retriever));
        }
        let retriever = Arc::new(Retriever {
            embedder: self.services.embedder(&self.config)?,
            items: ItemStore::connect(&self.config).map_err(|error| self.failure(error))?,
            concepts: ConceptStore::connect(&self.config).map_err(|error| self.failure(error))?,
            graph: self.connect_graph().await?,
        });
        *kept = Some((era, Arc::clone(&retriever)));
        Ok(retriever)
    }

    /// The graph alone, kept the same way: a read that needs no embedder, such as the concepts
    /// of a page, must not fail for a missing Gemini key.
    ///
    /// # Errors
    /// A failure that says the graph store is not ready.
    pub async fn graph(&self) -> Result<Arc<FalkorGraph>, Failure> {
        let mut kept = self.graph.lock().await;
        let era = self.era.load(Ordering::Acquire);
        if let Some((built, graph)) = kept.as_ref()
            && *built == era
        {
            return Ok(Arc::clone(graph));
        }
        let graph = Arc::new(self.connect_graph().await?);
        *kept = Some((era, Arc::clone(&graph)));
        Ok(graph)
    }

    async fn connect_graph(&self) -> Result<FalkorGraph, Failure> {
        match tokio::time::timeout(CONNECT_LIMIT, self.services.graph(&self.config)).await {
            Ok(connected) => connected.map_err(|failure| self.failure(failure)),
            Err(_) => {
                let address = &self.config.falkordb_url;
                Err(self.failure(Failure {
                    detail: format!(
                        "FalkorDB at {address} did not answer within {} seconds",
                        CONNECT_LIMIT.as_secs()
                    ),
                    ..Failure::falkordb_down(address)
                }))
            }
        }
    }

    /// Fresh stores, for one writing job.
    ///
    /// # Errors
    /// A failure that says which store is not ready.
    pub async fn stores(&self) -> Result<Stores, Failure> {
        Ok(Stores {
            items: ItemStore::connect(&self.config).map_err(|error| self.failure(error))?,
            graph: self.connect_graph().await?,
            concepts: ConceptStore::connect(&self.config).map_err(|error| self.failure(error))?,
        })
    }

    /// Converts the pages of a chapter that are not converted yet, with the Jev key of the
    /// settings, so no other code sees the key. Each page is told to `on_page` as it is cut out
    /// and as it ends.
    ///
    /// # Errors
    /// The errors of the conversion, such as a missing key or a page that fails.
    pub async fn convert_chapter(
        &self,
        job: &ChapterJob,
        on_page: &mut (dyn FnMut(PageProgress) + Send),
    ) -> Result<ConversionSummary, ConvertError> {
        let key = self.config.jev_api_key.as_ref().map(|key| key.expose());
        self.services.convert(job, key, on_page).await
    }

    /// Asks Google whether it accepts the key of the settings, so that no caller holds the key.
    /// `None` when the settings have no key.
    pub async fn check_gemini_key(&self) -> Option<Result<(), EmbedError>> {
        let key = self.config.gemini_api_key.as_ref()?;
        Some(self.services.check_gemini_key(key).await)
    }

    /// Asks Jev whether it accepts the key of the settings, so that no caller holds the key.
    /// `None` when the settings have no key.
    pub async fn check_jev_key(&self) -> Option<Result<(), JevError>> {
        let key = self.config.jev_api_key.as_ref()?;
        Some(self.services.check_jev_key(key).await)
    }

    /// Checks the agent of the Ingest choice without asking a question. The services can supply a
    /// stand-in.
    ///
    /// # Errors
    /// A failure that says what the person must do before that agent can answer.
    pub async fn ingest_agent_ready(&self) -> Result<(), Failure> {
        self.services
            .llm(WorkKind::Ingest)
            .check_ready()
            .await
            .map_err(|error| self.failure(error))
    }

    /// # Errors
    /// A failure that says the embedding key is missing.
    pub fn models(&self) -> Result<Models<S::Embedder, S::Llm>, Failure> {
        Ok(Models {
            embedder: self.services.embedder(&self.config)?,
            concepts: ConceptExtractor::new(self.services.llm(WorkKind::Ingest), &self.config),
        })
    }

    pub fn answer_llm(&self) -> S::Llm {
        self.services.llm(WorkKind::Ask)
    }

    pub fn dataset_llm(&self) -> S::Llm {
        self.services.llm(WorkKind::Datasets)
    }

    /// Every kind of work converts its errors through it. A store that is down also drops what
    /// was kept, so the next command connects again.
    pub fn failure(&self, error: impl Into<Failure>) -> Failure {
        let failure = error.into();
        if matches!(
            failure.kind,
            FailureKind::QdrantDown | FailureKind::FalkorDbDown
        ) {
            self.era.fetch_add(1, Ordering::AcqRel);
        }
        failure
    }
}
