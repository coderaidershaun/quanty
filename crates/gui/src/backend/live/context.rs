//! The settings and the outside services that every kind of work shares, and the one place that
//! turns a backend error into something a person can read.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use graph::FalkorGraph;
use ocr::convert::services::JevError;
use ocr::{ChapterJob, ConversionSummary, ConvertError, PageProgress};
use rag_core::{
    AgentCli, ApiKey, ConceptStore, Config, Database, EmbedError, Embedder, GeminiEmbedder,
    ItemStore, Llm, WorkKind,
};
use rag_ingestion::{ConceptExtractor, InsightsWriter, KeyClients, Models, Stores};
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
    /// The settings in use. A command takes the `Arc` out and works with that one, so a switch
    /// of the database never changes the settings under a command that has begun.
    config: RwLock<Arc<Config>>,
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
            config: RwLock::new(Arc::new(config)),
            services,
            retriever: tokio::sync::Mutex::new(None),
            graph: tokio::sync::Mutex::new(None),
            era: AtomicU64::new(0),
        }
    }

    pub fn config(&self) -> Arc<Config> {
        let config = self
            .config
            .read()
            .expect("the settings lock is never held while code that can panic runs");
        Arc::clone(&config)
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
        let config = self.config();
        let retriever = Arc::new(Retriever {
            embedder: self.services.embedder(&config)?,
            items: ItemStore::connect(&config).map_err(|error| self.failure(error))?,
            concepts: ConceptStore::connect(&config).map_err(|error| self.failure(error))?,
            graph: self.connect_graph(&config).await?,
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
        let config = self.config();
        let graph = Arc::new(self.connect_graph(&config).await?);
        *kept = Some((era, Arc::clone(&graph)));
        Ok(graph)
    }

    async fn connect_graph(&self, config: &Config) -> Result<FalkorGraph, Failure> {
        match tokio::time::timeout(CONNECT_LIMIT, self.services.graph(config)).await {
            Ok(connected) => connected.map_err(|failure| self.failure(failure)),
            Err(_) => Err(self.failure(did_not_answer(
                "FalkorDB",
                &config.falkordb_url,
                Failure::falkordb_down,
            ))),
        }
    }

    /// Makes `database` the one every command reads and writes, once both of its stores answer
    /// and, for a saved entry, its content folder is there. On any failure nothing changes.
    ///
    /// # Errors
    /// A failure that says which store is down, or that the content folder is not there.
    pub async fn use_database(&self, database: &Database) -> Result<(), Failure> {
        if !database.is_local() && !database.content_folder.is_dir() {
            return Err(content_folder_missing(&database.content_folder));
        }
        let config = Arc::new((*self.config()).clone().with_database(database));
        self.ping_qdrant(&config).await?;
        self.connect_graph(&config).await?;
        // WARNING: write the settings first and raise the era second, with no `.await` between.
        // `retriever()` and `graph()` read the era before they read the settings. In the other
        // order, one of them could read the new era with the old settings and keep a store of
        // the old database under the new era for ever.
        *self
            .config
            .write()
            .expect("the settings lock is never held while code that can panic runs") = config;
        self.era.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    async fn ping_qdrant(&self, config: &Config) -> Result<(), Failure> {
        let items = ItemStore::connect(config).map_err(|error| self.failure(error))?;
        match tokio::time::timeout(CONNECT_LIMIT, items.ping()).await {
            Ok(pinged) => pinged.map_err(|error| self.failure(error)),
            Err(_) => Err(self.failure(did_not_answer(
                "Qdrant",
                &config.qdrant_url,
                Failure::qdrant_down,
            ))),
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

/// A store that did not answer within the limit, which counts as that store being down.
fn did_not_answer(store: &str, address: &str, down: fn(&str) -> Failure) -> Failure {
    Failure {
        detail: format!(
            "{store} at {address} did not answer within {} seconds",
            CONNECT_LIMIT.as_secs()
        ),
        ..down(address)
    }
}

fn content_folder_missing(folder: &Path) -> Failure {
    let folder = folder.display();
    Failure::new(
        FailureKind::SourceMissing,
        format!("the content folder {folder} is not a folder"),
    )
    .with_hint(format!(
        "The content folder {folder} is not there. Mount it, then try again."
    ))
}
