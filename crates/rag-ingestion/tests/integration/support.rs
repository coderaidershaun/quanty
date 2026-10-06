//! Shared by the tests of this crate: where the committed chapters are, an embedder that makes
//! up vectors, and a collection, a graph and a cache folder that are removed when the test ends.

use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use graph::FalkorGraph;
use graph::testing::{
    GraphSize, StoredDocument, StoredItem, ThrowawayGraph, size, stored_document,
};
use qdrant_client::qdrant::ScrollPointsBuilder;
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::{Payload, Qdrant};
use rag_core::{
    Config, DocumentInput, EMBEDDING_DIMENSIONS, EmbedError, Embedder, Embedding, ItemStore, Llm,
};
use rag_ingestion::{ConceptExtractor, Item, Models, Stores};
use serde_json::Value;
use tempfile::TempDir;

const THROWAWAY_PREFIX: &str = "test-items-";

/// What the stand-in `claude` prints when the binary runs: no item discusses a concept.
const STAND_IN_ANSWER: &str = r#"{"type":"result","subtype":"success","is_error":false,"structured_output":{"concepts":[],"relations":[]},"total_cost_usd":0}"#;

fn content_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/content")
}

/// Seven pages converted from a real book: every kind of piece, headings, footnotes and page
/// breaks.
pub fn sample_chapter() -> PathBuf {
    content_folder().join("option-volatility-and-pricing/chapter-1")
}

/// Written by hand: options pricing at the level of intuition.
pub fn intuition_chapter() -> PathBuf {
    content_folder().join("quanty-sample-notes/chapter-1")
}

/// Written by hand: the derivation and the formulas of the Black–Scholes model.
pub fn in_depth_chapter() -> PathBuf {
    content_folder().join("quanty-sample-notes/chapter-2")
}

fn stand_in_claude_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/stand-in-claude")
}

/// Keeps every input it is given, and answers each with a vector that depends on the input, so
/// that different items get different points.
#[derive(Default)]
pub struct StandInEmbedder {
    received: Mutex<Vec<DocumentInput>>,
}

impl StandInEmbedder {
    pub fn received(&self) -> Vec<DocumentInput> {
        self.received.lock().unwrap().clone()
    }
}

impl Embedder for StandInEmbedder {
    async fn embed_document(&self, inputs: &[DocumentInput]) -> Result<Vec<Embedding>, EmbedError> {
        self.received.lock().unwrap().extend(inputs.iter().cloned());
        Ok(inputs
            .iter()
            .map(|input| unit_vector(&format!("{}|{}|{:?}", input.title, input.text, input.image)))
            .collect())
    }

    async fn embed_query(&self, query: &str) -> Result<Embedding, EmbedError> {
        Ok(unit_vector(query))
    }
}

fn unit_vector(seed_text: &str) -> Embedding {
    let mut hasher = DefaultHasher::new();
    seed_text.hash(&mut hasher);
    let mut state = hasher.finish();
    let numbers: Vec<f32> = (0..EMBEDDING_DIMENSIONS)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 40) as f32 / (1u64 << 24) as f32 - 0.5
        })
        .collect();
    let norm = numbers
        .iter()
        .map(|number| number * number)
        .sum::<f32>()
        .sqrt();
    numbers.into_iter().map(|number| number / norm).collect()
}

/// A config that stores into a collection, a graph and a cache folder that no one else uses. The
/// collection and the graph are deleted when this is dropped, and the cache folder goes with a
/// temporary folder, so they go away also when the test fails. It never names the real
/// collection, the real graph or the real cache folder, and every store a test touches is reached
/// through this config and from nowhere else.
pub struct ThrowawayStores {
    config: Config,
    temporary: TempDir,
    _graph: ThrowawayGraph,
}

/// Which `claude` program the `rag-ingest` command under test starts.
enum ClaudeProgram {
    /// The committed stand-in, which finds no concept and bills nothing.
    StandIn,
    /// The real one, on the subscription.
    Real,
}

impl ThrowawayStores {
    pub fn new(test_name: &str) -> Self {
        let nanoseconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let name = format!(
            "{THROWAWAY_PREFIX}{test_name}-{}-{nanoseconds}",
            std::process::id()
        );
        let settings = Config::load().expect("the settings should load");
        let graph = ThrowawayGraph::new(&settings, test_name);
        let temporary = tempfile::tempdir().expect("a temporary folder should be made");
        let config = Config {
            items_collection: name,
            falkordb_graph: graph.name().to_owned(),
            concept_cache_folder: temporary.path().join("cache"),
            ..settings
        };
        // The names are copied into the config by hand. Without these three checks, a copy that
        // is missing would leave the real names in the config, and the tests would write to the
        // real graph, the real collection and the real cache folder.
        assert_eq!(config.falkordb_graph, graph.name());
        assert!(config.items_collection.starts_with(THROWAWAY_PREFIX));
        assert!(config.concept_cache_folder.starts_with(temporary.path()));
        Self {
            config,
            temporary,
            _graph: graph,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Both stores, opened with this config.
    pub async fn connect(&self) -> Stores<FalkorGraph> {
        Stores {
            items: ItemStore::connect(&self.config).expect("the item store should open"),
            graph: FalkorGraph::connect(&self.config)
                .await
                .expect("FalkorDB should answer"),
        }
    }

    /// Both models of an ingest: an embedder that makes up vectors, and the concept extractor
    /// with this language model and the cache folder of this config. A test builds its models
    /// here and nowhere else, so it never names a cache folder by hand.
    pub fn models<L: Llm>(&self, llm: L) -> Models<StandInEmbedder, L> {
        Models {
            embedder: StandInEmbedder::default(),
            concepts: ConceptExtractor::new(llm, &self.config.concept_cache_folder),
        }
    }

    /// Runs the `rag-ingest` command against these stores and no others, with the stand-in
    /// `claude` that finds no concept. Every test that runs the command to ingest or to delete
    /// goes through here.
    pub fn rag_ingest(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output {
        self.run_rag_ingest(arguments, ClaudeProgram::StandIn)
    }

    /// Like [`ThrowawayStores::rag_ingest`], but the command starts the real `claude`, so it
    /// spends usage of the subscription. Only the live concept test calls it.
    pub fn rag_ingest_asking_claude(
        &self,
        arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Output {
        self.run_rag_ingest(arguments, ClaudeProgram::Real)
    }

    fn run_rag_ingest(
        &self,
        arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
        claude: ClaudeProgram,
    ) -> Output {
        let settings = [
            ("QDRANT_URL", self.config.qdrant_url.as_str()),
            ("FALKORDB_URL", self.config.falkordb_url.as_str()),
            (
                "QDRANT_ITEMS_COLLECTION",
                self.config.items_collection.as_str(),
            ),
            ("FALKORDB_GRAPH", self.config.falkordb_graph.as_str()),
            (
                "CONCEPT_CACHE_DIR",
                self.config
                    .concept_cache_folder
                    .to_str()
                    .expect("the cache folder should be UTF-8"),
            ),
        ];
        // The setting names are typed by hand. A mistyped name would make the command fall back
        // to the real collection, the real graph and the real cache folder, so read the names
        // back the way the command reads them before it starts.
        let read_back = Config::from_sources(
            |name| {
                settings
                    .iter()
                    .find(|(setting, _)| *setting == name)
                    .map(|(_, value)| (*value).to_owned())
            },
            None,
        )
        .expect("the settings should read back");
        assert_eq!(read_back.items_collection, self.config.items_collection);
        assert_eq!(read_back.falkordb_graph, self.config.falkordb_graph);
        assert_eq!(
            read_back.concept_cache_folder,
            self.config.concept_cache_folder
        );
        let mut command = Command::new(env!("CARGO_BIN_EXE_rag-ingest"));
        command.args(arguments).envs(settings);
        if let ClaudeProgram::StandIn = claude {
            let work = self.temporary.path().join("stand-in-claude");
            fs::create_dir_all(&work).expect("the stand-in folder should be made");
            fs::write(work.join("stdout"), STAND_IN_ANSWER)
                .expect("the stand-in answer should be written");
            let path = format!(
                "{}:{}",
                stand_in_claude_folder().display(),
                std::env::var("PATH").unwrap_or_default()
            );
            command.env("PATH", path).env("STAND_IN_CLAUDE", &work);
        }
        command
            .output()
            .expect("the rag-ingest binary should start")
    }
}

impl Drop for ThrowawayStores {
    fn drop(&mut self) {
        let name = self.config.items_collection.clone();
        if !name.starts_with(THROWAWAY_PREFIX) {
            return;
        }
        let url = self.config.qdrant_url.clone();
        let collection = name.clone();
        // A new thread with its own runtime, because a drop cannot wait inside the runtime of
        // the test.
        let removed = std::thread::spawn(move || -> anyhow::Result<()> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let client = Qdrant::from_url(&url).skip_compatibility_check().build()?;
                client.delete_collection(collection.as_str()).await?;
                Ok(())
            })
        })
        .join();
        match removed {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!("could not remove the throwaway collection {name}: {error:#}");
            }
            Err(_) => eprintln!(
                "could not remove the throwaway collection {name}: the thread that removes it panicked"
            ),
        }
    }
}

/// Every point of the collection the config names, as its identifier and its payload.
pub async fn points_in(config: &Config) -> Vec<(String, Value)> {
    let client = Qdrant::from_url(&config.qdrant_url)
        .skip_compatibility_check()
        .build()
        .unwrap();
    let reply = client
        .scroll(
            ScrollPointsBuilder::new(config.items_collection.as_str())
                .limit(1000)
                .with_payload(true)
                .with_vectors(false),
        )
        .await
        .unwrap();
    reply
        .result
        .into_iter()
        .map(|point| {
            let id = match point.id.and_then(|id| id.point_id_options) {
                Some(PointIdOptions::Uuid(id)) => id,
                other => panic!("a point id that is not a UUID: {other:?}"),
            };
            (id, Value::from(Payload::from(point.payload)))
        })
        .collect()
}

/// What the graph should hold for these items, in their reading order.
fn expected_stored_items(items: &[Item]) -> Vec<StoredItem> {
    items
        .iter()
        .map(|item| StoredItem {
            id: item.id.to_string(),
            kind: item.payload.kind.as_str().to_owned(),
            page: i64::from(item.payload.page),
            printed_page: item.payload.printed_page.clone(),
        })
        .collect()
}

/// The size of a graph that holds one document and these many items: the document and its
/// items are the nodes, and the edges are one `HAS_ITEM` for each item and one `NEXT` between
/// each two items that follow each other.
pub fn size_of_one_document(item_count: usize) -> GraphSize {
    let items = item_count as u64;
    GraphSize {
        nodes: items + 1,
        edges: items + items.saturating_sub(1),
    }
}

/// Checks that the graph holds the document of these items and each item with the values it was
/// stored with, in reading order. It does not look at the rest of the graph. Returns what the
/// graph holds for the document.
pub async fn assert_document_stored(graph: &FalkorGraph, items: &[Item]) -> StoredDocument {
    let payload = &items
        .first()
        .expect("a chapter should have at least one item")
        .payload;
    let stored = stored_document(graph, payload.doc_id)
        .await
        .expect("the graph should hold the document");
    assert_eq!(stored.title, payload.doc_title);
    assert_eq!(stored.items, expected_stored_items(items));
    stored
}

/// Checks that the graph holds these items and nothing else: their document, each item with the
/// values it was stored with, in reading order, and no other node or edge. Returns what the
/// graph holds for the document.
pub async fn assert_graph_holds_only(graph: &FalkorGraph, items: &[Item]) -> StoredDocument {
    let stored = assert_document_stored(graph, items).await;
    assert_eq!(size(graph).await, size_of_one_document(items.len()));
    stored
}
