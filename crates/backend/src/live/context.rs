//! The settings and the outside services that every kind of work shares, and the one place that
//! turns a backend error into something a person can read.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use graph::FalkorGraph;
use ocr::convert::services::JevError;
use ocr::{ChapterJob, ConversionSummary, ConvertError, PageProgress};
use rag_core::{
    AgentCli, ApiKey, ConceptStore, Config, EmbedError, Embedder, GeminiEmbedder, ItemStore, Llm,
    WorkKind,
};
use rag_ingestion::{ConceptExtractor, InsightsWriter, KeyClients, Models, Stores};
use rag_retrieval::Retriever;

use contract::{Failure, FailureKind};

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

    /// Asks Google whether it accepts `key`. It costs nothing, and it cannot see spent credit. The
    /// real service makes its client when this is called and not when the future runs, so that a
    /// time limit on the future never covers it. The key is given by value because the future
    /// would otherwise keep a borrow of the settings it was read from.
    fn check_gemini_key(&self, key: ApiKey) -> impl Future<Output = Result<(), EmbedError>> + Send;

    /// Asks Jev whether it accepts `key`. It costs nothing, and Jev documents no reply to it for
    /// spent credit.
    fn check_jev_key(&self, key: ApiKey) -> impl Future<Output = Result<(), JevError>> + Send;
}

/// The real services. It keeps the clients that ask the providers about a key, so each is made
/// once.
#[derive(Default)]
pub struct RealServices {
    key_clients: KeyClients,
}

impl Services for RealServices {
    type Embedder = GeminiEmbedder;
    type Llm = AgentCli;

    fn embedder(&self, config: &Config) -> Result<GeminiEmbedder, Failure> {
        GeminiEmbedder::from_config(config).map_err(super::failure::IntoFailure::into_failure)
    }

    fn llm(&self, work: WorkKind) -> AgentCli {
        AgentCli::for_work(work)
    }

    async fn graph(&self, config: &Config) -> Result<FalkorGraph, Failure> {
        FalkorGraph::connect(config)
            .await
            .map_err(super::failure::IntoFailure::into_failure)
    }

    async fn convert(
        &self,
        job: &ChapterJob,
        jev_api_key: Option<&str>,
        on_page: &mut (dyn FnMut(PageProgress) + Send),
    ) -> Result<ConversionSummary, ConvertError> {
        ocr::convert_chapter_with_jev_key(job, jev_api_key, on_page).await
    }

    fn check_gemini_key(&self, key: ApiKey) -> impl Future<Output = Result<(), EmbedError>> + Send {
        let client = self.key_clients.gemini(&key);
        async move { client?.check_key().await }
    }

    fn check_jev_key(&self, key: ApiKey) -> impl Future<Output = Result<(), JevError>> + Send {
        let client = self.key_clients.jev(&key);
        async move { client?.check_key().await }
    }
}

/// The retriever that a context keeps once the stores answered. Without this name clippy's
/// `type_complexity` fails the field that holds it.
pub type Kept<S> = Arc<Retriever<<S as Services>::Embedder>>;

pub struct LiveContext<S: Services> {
    home: PathBuf,
    config: Arc<Config>,
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
            home: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            config: Arc::new(config),
            services,
            retriever: tokio::sync::Mutex::new(None),
            graph: tokio::sync::Mutex::new(None),
            era: AtomicU64::new(0),
        }
    }

    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = home.into();
        self
    }

    pub fn home(&self) -> &std::path::Path {
        &self.home
    }

    pub fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    /// # Errors
    /// A failure that says which store or key is not ready.
    pub async fn retriever(&self) -> Result<Kept<S>, Failure> {
        self.kept(&self.retriever, async {
            let config = self.config();
            Ok(Retriever {
                embedder: self.services.embedder(&config)?,
                items: ItemStore::connect(&config).map_err(|error| self.failure(error))?,
                concepts: ConceptStore::connect(&config).map_err(|error| self.failure(error))?,
                graph: self.connect_graph(&config).await?,
            })
        })
        .await
    }

    /// The graph alone, kept the same way: a read that needs no embedder, such as the concepts
    /// of a page, must not fail for a missing Gemini key.
    ///
    /// # Errors
    /// A failure that says the graph store is not ready.
    pub async fn graph(&self) -> Result<Arc<FalkorGraph>, Failure> {
        self.kept(&self.graph, async {
            self.connect_graph(&self.config()).await
        })
        .await
    }

    /// What `slot` kept in the current era, or else what `build` makes, which is then kept.
    async fn kept<T>(
        &self,
        slot: &tokio::sync::Mutex<Option<(u64, Arc<T>)>>,
        build: impl Future<Output = Result<T, Failure>>,
    ) -> Result<Arc<T>, Failure> {
        let mut kept = slot.lock().await;
        let era = self.era.load(Ordering::Acquire);
        if let Some((built, value)) = kept.as_ref()
            && *built == era
        {
            return Ok(Arc::clone(value));
        }
        let value = Arc::new(build.await?);
        *kept = Some((era, Arc::clone(&value)));
        Ok(value)
    }

    async fn connect_graph(&self, config: &Config) -> Result<FalkorGraph, Failure> {
        match tokio::time::timeout(CONNECT_LIMIT, self.services.graph(config)).await {
            Ok(connected) => connected.map_err(|failure| self.failure(failure)),
            Err(_) => Err(self.failure(Failure {
                detail: format!(
                    "FalkorDB at {} did not answer within {} seconds",
                    config.falkordb_url,
                    CONNECT_LIMIT.as_secs()
                ),
                ..Failure::falkordb_down(&config.falkordb_url)
            })),
        }
    }

    /// Fresh stores, for one writing job.
    ///
    /// # Errors
    /// A failure that says which store is not ready.
    pub async fn stores(&self) -> Result<Stores, Failure> {
        let config = self.config();
        Ok(Stores {
            items: ItemStore::connect(&config).map_err(|error| self.failure(error))?,
            graph: self.connect_graph(&config).await?,
            concepts: ConceptStore::connect(&config).map_err(|error| self.failure(error))?,
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
        let config = self.config();
        let key = config.jev_api_key.as_ref().map(ApiKey::expose);
        self.services.convert(job, key, on_page).await
    }

    /// Asks Google whether it accepts the key of the settings, so that no caller holds the key.
    /// `None` when the settings have no key. The client is made when this is called, and the
    /// future is only the read, so a time limit around it never covers the making.
    pub fn check_gemini_key(&self) -> Option<impl Future<Output = Result<(), EmbedError>> + Send> {
        let key = self.config().gemini_api_key.clone()?;
        Some(self.services.check_gemini_key(key))
    }

    /// Asks Jev whether it accepts the key of the settings, so that no caller holds the key.
    /// `None` when the settings have no key. The client is made when this is called, like the one
    /// for Google.
    pub fn check_jev_key(&self) -> Option<impl Future<Output = Result<(), JevError>> + Send> {
        let key = self.config().jev_api_key.clone()?;
        Some(self.services.check_jev_key(key))
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
        let config = self.config();
        Ok(Models {
            artifacts: rag_ingestion::ArtifactWriter::new(self.services.llm(WorkKind::Ingest)),
            embedder: self.services.embedder(&config)?,
            concepts: ConceptExtractor::new(self.services.llm(WorkKind::Ingest), &config),
            insights: Some(InsightsWriter::new(self.services.llm(WorkKind::Ingest))),
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
    pub fn failure(&self, error: impl super::failure::IntoFailure) -> Failure {
        let failure = error.into_failure();
        if matches!(
            failure.kind,
            FailureKind::QdrantDown | FailureKind::FalkorDbDown
        ) {
            self.era.fetch_add(1, Ordering::AcqRel);
        }
        failure
    }
}
