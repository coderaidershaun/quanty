//! What the tests of the live backend share: stand-in services over throwaway stores, the
//! committed sample chapters stored in them, with the paid page services stubbed, and a config
//! whose stores cannot be reached.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use graph::{ConceptNode, FalkorGraph, GraphStore, Mention, Relation, RelationKind};
use gui::backend::live::{LiveContext, RealServices, Services};
use gui::contract::{DocId, Failure};
use ocr::testing::{Scenario, StubServices};
use ocr::{ChapterIndex, ConvertError};
use rag_core::{ApiKey, Config, ItemKind};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores, first_axis, vector_at};
use rag_ingestion::{Item, chapter_items, ingest_chapter};
use serde_json::json;

pub const QUESTION: &str = "How is a call option priced?";
pub const IN_DEPTH: &str = "quanty-sample-notes/chapter-2";
pub const INTUITION: &str = "quanty-sample-notes/chapter-1";
pub const SAMPLE_PAGES: &str = "option-volatility-and-pricing/chapter-1";

/// Services that never reach a real store or a paid model. The graph is the throwaway graph of
/// `stores`, and `pages` is kept so a test can read the calls the stub received.
pub struct StandInServices {
    pub stores: ThrowawayStores,
    pub embedder: Box<dyn Fn() -> StandInEmbedder + Send + Sync>,
    pub llm: Box<dyn Fn(&str) -> StandInLlm + Send + Sync>,
    pub pages: Arc<StubServices>,
}

impl Services for StandInServices {
    type Embedder = StandInEmbedder;
    type Llm = StandInLlm;
    type Graph = FalkorGraph;
    type Pages = StubServices;

    fn embedder(&self, _config: &Config) -> Result<StandInEmbedder, Failure> {
        Ok((self.embedder)())
    }

    fn llm(&self, model: &str) -> StandInLlm {
        (self.llm)(model)
    }

    async fn graph(&self, _config: &Config) -> Result<FalkorGraph, Failure> {
        Ok(FalkorGraph::connect(self.stores.config()).await?)
    }

    async fn page_services(
        &self,
        _jev_api_key: Option<&str>,
    ) -> Result<Arc<StubServices>, ConvertError> {
        Ok(Arc::clone(&self.pages))
    }
}

/// A context over throwaway stores. The embedder makes up its vectors, the model finds no
/// concept, and the page services are stubs of the sample chapter.
pub fn context(test_name: &str) -> LiveContext<StandInServices> {
    let stores = ThrowawayStores::new(test_name);
    let config = stores.config().clone();
    let services = StandInServices {
        stores,
        embedder: Box::new(StandInEmbedder::default),
        llm: Box::new(|_model| StandInLlm::finding_nothing()),
        pages: Arc::new(StubServices::new(Scenario::SampleChapter)),
    };
    LiveContext::new(config, services)
}

/// A config whose Qdrant and FalkorDB addresses nothing listens on, so a store is always down.
pub fn closed_ports_config() -> Config {
    let folder = std::env::temp_dir().join("quanty-closed-ports");
    Config {
        qdrant_url: "http://127.0.0.1:1".to_owned(),
        falkordb_url: "falkor://127.0.0.1:1".to_owned(),
        falkordb_graph: "closed-ports".to_owned(),
        items_collection: "closed-ports-items".to_owned(),
        concepts_collection: "closed-ports-concepts".to_owned(),
        concept_cache_folder: folder.join("cache"),
        concept_decision_log: folder.join("decisions.jsonl"),
        content_folder: folder.join("content"),
        gemini_api_key: Some(ApiKey::new("a-made-up-key")),
        jev_api_key: None,
    }
}

/// The real services on closed ports, with `content_folder` as the folder of the books.
pub fn closed(content_folder: &Path) -> LiveContext<RealServices> {
    let config = Config {
        content_folder: content_folder.to_path_buf(),
        ..closed_ports_config()
    };
    LiveContext::new(config, RealServices)
}

/// A committed chapter, as the path that the page carries: ingestion keeps the canonical path of
/// a chapter folder, and the path of a test does not depend on where it runs from.
pub fn sample_chapter(chapter: &str) -> PathBuf {
    let folder = gui::testkit::samples_folder().join(chapter);
    std::fs::canonicalize(folder).expect("a committed chapter should exist")
}

/// The id that the stores give the document of a saved chapter.
pub fn document_of(chapter: &Path) -> DocId {
    let index = ChapterIndex::read(chapter).expect("the chapter index should be read");
    rag_core::DocId::from_source_sha256(&index.source_sha256).into()
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

/// A context over `stores` whose answers come from `answering`.
pub fn context_with(
    stores: ThrowawayStores,
    answering: &StandInLlm,
    embedder: impl Fn() -> StandInEmbedder + Send + Sync + 'static,
) -> LiveContext<StandInServices> {
    let config = stores.config().clone();
    context_over(config, stores, answering, embedder)
}

/// The same, with the settings of the context given: the graph is still the one of `stores`.
pub fn context_over(
    config: Config,
    stores: ThrowawayStores,
    answering: &StandInLlm,
    embedder: impl Fn() -> StandInEmbedder + Send + Sync + 'static,
) -> LiveContext<StandInServices> {
    let answering = answering.clone();
    let services = StandInServices {
        stores,
        embedder: Box::new(embedder),
        llm: Box::new(move |_model| answering.clone()),
        pages: Arc::new(StubServices::new(Scenario::SampleChapter)),
    };
    LiveContext::new(config, services)
}

/// How the three sample chapters are found again after they are stored. The graph keeps the
/// folder of `stored`. `copied` is copied under the content folder, so only a scan finds it. The
/// third chapter has neither.
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

/// Ingests the three committed chapters into throwaway stores, with the picks placed near the
/// question, then writes by hand what no stand-in finds: two concepts, what mentions one of
/// them, and the relation between them. The chapters are found again as `folders` says.
pub async fn fill(name: &str, answering: &StandInLlm, folders: Folders) -> World {
    let stores = ThrowawayStores::new(name);
    let connected = stores.connect().await;
    let picks = Picks::of_the_samples();
    let models = stores.models_with_embedder(picks.embedder(), StandInLlm::finding_nothing());
    let mut stored_document = None;
    for chapter in [SAMPLE_PAGES, INTUITION, IN_DEPTH] {
        let summary = ingest_chapter(&sample_chapter(chapter), &models, &connected)
            .await
            .expect("a sample chapter should be ingested");
        if chapter == folders.stored {
            stored_document = Some(summary.doc_id);
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
        .set_chapter_folder(stored_document, &sample_chapter(folders.stored))
        .await
        .expect("the folder should be stored");
    let content = &stores.config().content_folder;
    copy_folder(
        &sample_chapter(folders.copied),
        &content.join(folders.copied),
    );

    let placing = picks.clone();
    World {
        cx: context_with(stores, answering, move || placing.embedder()),
        picks,
        black_scholes,
        lemma,
    }
}

/// A context over throwaway stores that hold the three sample chapters, each found in another
/// way: the figure's chapter by the folder the graph keeps, Notes chapter 2 by a scan of the
/// content folder, and Notes chapter 1 not at all. The answer model cites the formula, the table
/// and the figure in its first claim, so the three chips are on the first screen of the answer,
/// and gives one follow-up.
pub async fn seeded(test_name: &str) -> LiveContext<StandInServices> {
    let answering = StandInLlm::replying(|_, _| {
        Ok(json!({
            "title": "How a call is priced",
            "claims": [
                { "heading": "", "text": "A call is priced by the formula.", "sources": [1, 2, 4] },
                { "heading": "Key assumptions", "text": "The price assumes a table.", "sources": [2] },
            ],
            "follow_ups": ["What is a put?"],
        }))
    });
    let folders = Folders {
        stored: SAMPLE_PAGES,
        copied: IN_DEPTH,
    };
    fill(test_name, &answering, folders).await.cx
}
