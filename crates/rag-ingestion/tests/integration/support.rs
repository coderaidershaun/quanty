//! Shared by the tests of this crate: where the committed chapters are, an embedder that makes
//! up vectors, and a collection and a graph that are removed when the test ends.

use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
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
    Config, DocumentInput, EMBEDDING_DIMENSIONS, EmbedError, Embedder, Embedding, ItemStore,
};
use rag_ingestion::{Item, Stores};
use serde_json::Value;

const THROWAWAY_PREFIX: &str = "test-items-";

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

/// A config that stores into a collection and a graph that no one else uses. The collection and
/// the graph are deleted when this is dropped, so they go away also when the test fails. It never
/// names the real collection or the real graph, and every store a test touches is reached through
/// this config and from nowhere else.
pub struct ThrowawayStores {
    config: Config,
    _graph: ThrowawayGraph,
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
        let config = Config {
            items_collection: name,
            falkordb_graph: graph.name().to_owned(),
            ..settings
        };
        // The names are copied into the config by hand. Without these two checks, a copy that is
        // missing would leave the real names in the config, and the tests would write to the real
        // graph and the real collection.
        assert_eq!(config.falkordb_graph, graph.name());
        assert!(config.items_collection.starts_with(THROWAWAY_PREFIX));
        Self {
            config,
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

    /// Runs the `rag-ingest` command against these stores and no others. Every test that runs
    /// the command to ingest or to delete goes through here.
    pub fn rag_ingest(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output {
        let settings = [
            ("QDRANT_URL", self.config.qdrant_url.as_str()),
            ("FALKORDB_URL", self.config.falkordb_url.as_str()),
            (
                "QDRANT_ITEMS_COLLECTION",
                self.config.items_collection.as_str(),
            ),
            ("FALKORDB_GRAPH", self.config.falkordb_graph.as_str()),
        ];
        // The setting names are typed by hand. A mistyped name would make the command fall back
        // to the real collection and the real graph, so read the names back the way the command
        // reads them before it starts.
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
        Command::new(env!("CARGO_BIN_EXE_rag-ingest"))
            .args(arguments)
            .envs(settings)
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

/// Checks that the graph holds these items and nothing else: their document, each item with the
/// values it was stored with, in reading order, and no other node or edge. Returns what the
/// graph holds for the document.
pub async fn assert_graph_holds_only(graph: &FalkorGraph, items: &[Item]) -> StoredDocument {
    let payload = &items
        .first()
        .expect("a chapter should have at least one item")
        .payload;
    let stored = stored_document(graph, payload.doc_id)
        .await
        .expect("the graph should hold the document");
    assert_eq!(stored.title, payload.doc_title);
    assert_eq!(stored.items, expected_stored_items(items));
    assert_eq!(size(graph).await, size_of_one_document(items.len()));
    stored
}
