//! What the tests of the live backend share: stand-in services over throwaway stores, the sample
//! chapters stored in them, and a config whose stores cannot be reached.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use backend::live::{LiveContext, RealServices, Services};
use backend::{Handler, Reply};
use contract::{Command, DocId, Event, Failure};
use graph::{ConceptNode, FalkorGraph, Mention, Relation, RelationKind};
use ocr::convert::services::JevError;
use ocr::testing::{Scenario, StubServices};
use ocr::{ChapterIndex, ChapterJob, ConversionSummary, ConvertError, PageProgress};
use rag_core::{ApiKey, Config, EmbedError, ItemKind, MediaLabels, WorkKind};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores, first_axis, vector_at};
use rag_ingestion::{ChapterFolder, Item, Models, chapter_items, ingest_chapter};

use crate::key_checks::{Check, KeyChecks};

pub const QUESTION: &str = "How is a call option priced?";
pub const IN_DEPTH: &str = "quanty-sample-notes/chapter-2";
pub const INTUITION: &str = "quanty-sample-notes/chapter-1";
pub const SAMPLE_PAGES: &str = "option-volatility-and-pricing/chapter-1";

/// Every throwaway graph has a name that starts with this, and the real graph never does.
const THROWAWAY_GRAPH_PREFIX: &str = "test-graph-";

/// The graph is the throwaway graph of `stores`, and `pages` is kept so a test can read the calls
/// the stub received.
pub struct StandInServices {
    pub stores: ThrowawayStores,
    pub embedder: Box<dyn Fn() -> StandInEmbedder + Send + Sync>,
    pub llm: Box<dyn Fn(WorkKind) -> StandInLlm + Send + Sync>,
    pub pages: Arc<StubServices>,
}

impl Services for StandInServices {
    type Embedder = StandInEmbedder;
    type Llm = StandInLlm;

    fn embedder(&self, _config: &Config) -> Result<StandInEmbedder, Failure> {
        Ok((self.embedder)())
    }

    fn llm(&self, work: WorkKind) -> StandInLlm {
        (self.llm)(work)
    }

    async fn graph(&self, _config: &Config) -> Result<FalkorGraph, Failure> {
        FalkorGraph::connect(self.stores.config())
            .await
            .map_err(backend::live::IntoFailure::into_failure)
    }

    async fn convert(
        &self,
        job: &ChapterJob,
        _jev_api_key: Option<&str>,
        on_page: &mut (dyn FnMut(PageProgress) + Send),
    ) -> Result<ConversionSummary, ConvertError> {
        ocr::convert::convert_chapter_with_progress(job, &*self.pages, on_page).await
    }

    async fn check_gemini_key(&self, _key: ApiKey, _ipv4_only: bool) -> Result<(), EmbedError> {
        Ok(())
    }

    async fn check_jev_key(&self, _key: ApiKey) -> Result<(), JevError> {
        Ok(())
    }
}

pub fn context(test_name: &str) -> LiveContext<StandInServices> {
    context_and_pages(test_name).0
}

/// The models of the live backend with no insights writer. A test that ingests the committed
/// samples where they are uses these, so that no insights file is written into the samples.
pub fn models_without_insights(
    cx: &LiveContext<StandInServices>,
) -> Models<StandInEmbedder, StandInLlm> {
    Models {
        insights: None,
        ..cx.models().expect("the models should be made")
    }
}

/// Also returns the stub pages, so a test can read the calls they got.
pub fn context_and_pages(test_name: &str) -> (LiveContext<StandInServices>, Arc<StubServices>) {
    let stores = ThrowawayStores::new(test_name);
    let config = stores.config().clone();
    let home = stores.temporary_folder().to_path_buf();
    let pages = Arc::new(StubServices::new(Scenario::SampleChapter));
    let services = StandInServices {
        stores,
        embedder: Box::new(StandInEmbedder::default),
        llm: Box::new(|_work| StandInLlm::finding_nothing()),
        pages: Arc::clone(&pages),
    };
    (LiveContext::new(config, services).with_home(home), pages)
}

/// Stand-ins for the paid services, and a graph that is the real one at the address of the
/// settings. With `closed_ports_config` the graph always refuses.
pub struct StandInsOnClosedPorts {
    pub pages: Arc<StubServices>,
}

impl Services for StandInsOnClosedPorts {
    type Embedder = StandInEmbedder;
    type Llm = StandInLlm;

    fn embedder(&self, _config: &Config) -> Result<StandInEmbedder, Failure> {
        Ok(StandInEmbedder::default())
    }

    fn llm(&self, _work: WorkKind) -> StandInLlm {
        StandInLlm::finding_nothing()
    }

    async fn graph(&self, config: &Config) -> Result<FalkorGraph, Failure> {
        FalkorGraph::connect(config)
            .await
            .map_err(backend::live::IntoFailure::into_failure)
    }

    async fn convert(
        &self,
        job: &ChapterJob,
        _jev_api_key: Option<&str>,
        on_page: &mut (dyn FnMut(PageProgress) + Send),
    ) -> Result<ConversionSummary, ConvertError> {
        ocr::convert::convert_chapter_with_progress(job, &*self.pages, on_page).await
    }

    async fn check_gemini_key(&self, _key: ApiKey, _ipv4_only: bool) -> Result<(), EmbedError> {
        Ok(())
    }

    async fn check_jev_key(&self, _key: ApiKey) -> Result<(), JevError> {
        Ok(())
    }
}

/// The app after it is started again: a second context made from a throwaway one. It shares
/// nothing with the first but the settings, so it reads the same throwaway graph and holds nothing
/// that the first one kept in memory.
///
/// # Panics
/// When the settings of `first` do not name a throwaway graph: the second context connects to
/// the graph that the settings name, and that must never be the real one.
pub fn started_again(first: &LiveContext<StandInServices>) -> LiveContext<StandInsOnClosedPorts> {
    let config = (*first.config()).clone();
    assert!(
        config.falkordb_graph.starts_with(THROWAWAY_GRAPH_PREFIX),
        "the graph `{}` is not a throwaway graph",
        config.falkordb_graph
    );
    let services = StandInsOnClosedPorts {
        pages: Arc::new(StubServices::new(Scenario::SampleChapter)),
    };
    LiveContext::new(config, services).with_home(first.home().to_path_buf())
}

/// A config whose Qdrant and FalkorDB addresses nothing listens on, so a store is always down.
pub fn closed_ports_config() -> Config {
    let folder = std::env::temp_dir().join("quanty-closed-ports");
    Config {
        qdrant_url: "http://127.0.0.1:1".to_owned(),
        qdrant_api_key: None,
        falkordb_url: "falkor://127.0.0.1:1".to_owned(),
        falkordb_password: None,
        falkordb_graph: "closed-ports".to_owned(),
        items_collection: "closed-ports-items".to_owned(),
        concepts_collection: "closed-ports-concepts".to_owned(),
        concept_cache_folder: folder.join("cache"),
        concept_decision_log: folder.join("decisions.jsonl"),
        content_folder: folder.join("content"),
        dataset_folder: folder.join("datasets"),
        dataset_cache_folder: folder.join("dataset-cache"),
        gemini_api_key: Some(ApiKey::new("a-made-up-key")),
        jev_api_key: None,
        gemini_ipv4_only: false,
    }
}

/// The real services over stores that are down, with key checks that find the provider
/// unreachable, so that no test reaches a provider.
pub fn closed(content_folder: &Path) -> LiveContext<KeyChecks<RealServices>> {
    let config = Config {
        content_folder: content_folder.to_path_buf(),
        ..closed_ports_config()
    };
    let services = KeyChecks {
        inner: RealServices::default(),
        gemini: Check::Unreachable,
        jev: Check::Unreachable,
    };
    LiveContext::new(config, services)
}

/// A committed chapter, as the path that the page carries: ingestion keeps the canonical path of
/// a chapter folder, and the path of a test does not depend on where it runs from.
pub fn sample_chapter(chapter: &str) -> PathBuf {
    let folder = gui::testkit::samples_folder().join(chapter);
    std::fs::canonicalize(folder).expect("a committed chapter should exist")
}

/// Every event that `backend` sends for `command`, in the order it sends them.
pub async fn served(backend: &impl Handler, command: Command) -> Vec<Event> {
    let (reply, events, _stop) = Reply::collecting();
    backend.serve(command, reply).await;
    events.try_iter().collect()
}

pub fn document_of(chapter: &Path) -> DocId {
    let index = ChapterIndex::read(chapter).expect("the chapter index should be read");
    DocId(rag_core::DocId::from_source_sha256(&index.source_sha256).0)
}

/// Four stored items, from nearest to the question to farthest.
#[derive(Clone)]
pub struct Picks {
    pub formula: Item,
    pub table: Item,
    pub chunk: Item,
    pub figure: Item,
}

impl Picks {
    fn of_the_samples() -> Picks {
        Picks {
            formula: pick(IN_DEPTH, ItemKind::Formula, Some("(2.4)")),
            table: pick(INTUITION, ItemKind::Table, Some("Table 1-1")),
            chunk: pick(IN_DEPTH, ItemKind::Chunk, None),
            figure: pick(SAMPLE_PAGES, ItemKind::Figure, Some("Figure 13-4")),
        }
    }

    /// An embedder that puts the question on the first axis and each pick at its own distance
    /// from it. Every other item gets a made-up vector that is near no pick.
    fn embedder(&self) -> StandInEmbedder {
        StandInEmbedder::default()
            .placing(QUESTION, first_axis())
            .placing(&self.formula.input.text, vector_at(0.9, 1))
            .placing(&self.table.input.text, vector_at(0.8, 2))
            .placing(&self.chunk.input.text, vector_at(0.7, 3))
            .placing(&self.figure.input.text, vector_at(0.6, 4))
    }
}

fn pick(chapter: &str, kind: ItemKind, label: Option<&str>) -> Item {
    let read = ocr::read_chapter(&sample_chapter(chapter)).expect("the chapter should be read");
    chapter_items(&read)
        .into_iter()
        .find(|item| item.payload.kind == kind && item.payload.label.as_deref() == label)
        .unwrap_or_else(|| panic!("{chapter} should have a {kind:?} labelled {label:?}"))
}

pub fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a folder should be made");
    for entry in std::fs::read_dir(from).expect("the folder should be listed") {
        let entry = entry.expect("an entry should be read");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("a file should be copied");
        }
    }
}

pub fn context_with(
    stores: ThrowawayStores,
    answering: &StandInLlm,
    embedder: impl Fn() -> StandInEmbedder + Send + Sync + 'static,
) -> LiveContext<StandInServices> {
    let config = stores.config().clone();
    context_over(config, stores, answering, embedder)
}

/// The settings of the context are given, but the graph is still the one of `stores`. `answering`
/// answers the ask; every other kind of work gets a signed-out model.
pub fn context_over(
    config: Config,
    stores: ThrowawayStores,
    answering: &StandInLlm,
    embedder: impl Fn() -> StandInEmbedder + Send + Sync + 'static,
) -> LiveContext<StandInServices> {
    let home = stores.temporary_folder().to_path_buf();
    let answering = answering.clone();
    let services = StandInServices {
        stores,
        embedder: Box::new(embedder),
        llm: Box::new(move |work| match work {
            WorkKind::Ask => answering.clone(),
            _ => StandInLlm::signed_out(),
        }),
        pages: Arc::new(StubServices::new(Scenario::SampleChapter)),
    };
    LiveContext::new(config, services).with_home(home)
}

/// How the three sample chapters are found again: the graph keeps the folder of `stored`, only a
/// scan of the content folder finds `copied`, and the third chapter has neither.
#[derive(Clone, Copy)]
pub struct Folders {
    pub stored: &'static str,
    pub copied: &'static str,
}

pub struct World {
    pub cx: LiveContext<StandInServices>,
    pub picks: Picks,
    pub black_scholes: ConceptNode,
    pub lemma: ConceptNode,
}

/// Two concepts, the mentions of one of them and the relation between them are written by hand,
/// because the stand-in model finds none.
pub async fn fill(name: &str, answering: &StandInLlm, folders: Folders) -> World {
    let stores = ThrowawayStores::new(name);
    let connected = stores.connect().await;
    let picks = Picks::of_the_samples();
    let models = stores.models_with_embedder(picks.embedder(), StandInLlm::finding_nothing());
    let mut stored_document = None;
    for chapter in [SAMPLE_PAGES, INTUITION, IN_DEPTH] {
        let copied = stores.config().content_folder.join(chapter);
        copy_folder(&sample_chapter(chapter), &copied);
        let folder = ChapterFolder {
            folder: &copied,
            new_media: &MediaLabels::default(),
        };
        let summary = ingest_chapter(folder, &models, &connected)
            .await
            .expect("a sample chapter should be ingested");
        if chapter == folders.stored {
            stored_document = Some(summary.doc_id);
        }
        if chapter != folders.stored && chapter != folders.copied {
            std::fs::remove_file(copied.join("chapter.json")).unwrap();
        }
    }

    let concept = |name: &str, definition: &str| ConceptNode {
        id: rag_core::ConceptId::random(),
        name: name.to_owned(),
        normalised_name: name.to_lowercase(),
        definition: definition.to_owned(),
    };
    let black_scholes = concept("Black–Scholes model", "The model of an option's price.");
    let lemma = concept("Itô's lemma", "How a function of a random walk changes.");
    let graph = &connected.graph;
    for concept in [&black_scholes, &lemma] {
        let stored = graph.upsert_concept(concept).await;
        stored.expect("a concept should be stored");
    }
    let mentioning = |item: &Item| Mention {
        item: item.id,
        concept: black_scholes.id,
        wording: black_scholes.name.clone(),
    };
    let mentions = [mentioning(&picks.formula), mentioning(&picks.chunk)];
    graph
        .add_mentions(&mentions)
        .await
        .expect("the mentions should be stored");
    let derived = Relation {
        from: black_scholes.id,
        to: lemma.id,
        kind: RelationKind::DerivedFrom,
        item: picks.formula.id,
    };
    graph
        .add_relations(&[derived])
        .await
        .expect("the relation should be stored");
    let stored_document = stored_document.expect("the stored chapter is one of the samples");
    graph
        .set_chapter_folder(
            stored_document,
            &stores.config().content_folder.join(folders.stored),
        )
        .await
        .expect("the folder should be stored");

    // Ingestion keeps the canonical path of a chapter folder, so a picture is inside the content
    // folder only when the folder that the settings name is canonical too.
    let config = Config {
        content_folder: std::fs::canonicalize(&stores.config().content_folder)
            .expect("the content folder should exist"),
        ..stores.config().clone()
    };
    let placing = picks.clone();
    World {
        cx: context_over(config, stores, answering, move || placing.embedder()),
        picks,
        black_scholes,
        lemma,
    }
}
