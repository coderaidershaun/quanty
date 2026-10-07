//! Shared by the tests of this crate: where the samples are, stores and folders that are removed
//! when the test ends, the stand-ins for the paid calls, and reads of the decision log.

mod decisions;
mod stand_in_embedder;
mod stand_in_image_services;
mod stand_in_llm;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use graph::FalkorGraph;
use graph::testing::{
    GraphSize, StoredDocument, StoredItem, ThrowawayGraph, size, stored_document,
};
use qdrant_client::qdrant::ScrollPointsBuilder;
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::{Payload, Qdrant};
use rag_core::{ConceptStore, Config, ItemStore, Llm};
use rag_ingestion::{ConceptExtractor, Item, Models, Stores};
use serde_json::Value;
use tempfile::TempDir;

pub use decisions::{decision_for_mention, decisions_in, mentions_of};
pub use stand_in_embedder::{StandInEmbedder, first_axis, vector_at};
pub use stand_in_image_services::{CAPTION, LABEL, StandInImageServices};
pub use stand_in_llm::StandInLlm;

const THROWAWAY_PREFIX: &str = "test-items-";
const THROWAWAY_CONCEPTS_PREFIX: &str = "test-concepts-";

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

/// A chart of a volatility surface, cut from a page of a book on option trading.
pub fn sample_picture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/images/volatility-surface.png")
}

fn stand_in_claude_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/stand-in-claude")
}

/// A config that stores into two collections, a graph, a cache folder, a decision log and a
/// content folder that no one else uses. The collections and the graph are deleted when this is
/// dropped, and the folders and the log go with a temporary folder, so they go away also when the
/// test fails. It never names a real collection, the real graph, the real cache folder, the real
/// decision log or the real content folder, and every store a test touches is reached through
/// this config and from nowhere else.
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
        let stamp = format!("{test_name}-{}-{nanoseconds}", std::process::id());
        let settings = Config::load().expect("the settings should load");
        let graph = ThrowawayGraph::new(&settings, test_name);
        let temporary = tempfile::tempdir().expect("a temporary folder should be made");
        let config = Config {
            items_collection: format!("{THROWAWAY_PREFIX}{stamp}"),
            concepts_collection: format!("{THROWAWAY_CONCEPTS_PREFIX}{stamp}"),
            falkordb_graph: graph.name().to_owned(),
            concept_cache_folder: temporary.path().join("cache"),
            concept_decision_log: temporary.path().join("concept-decisions.jsonl"),
            content_folder: temporary.path().join("content"),
            ..settings
        };
        // The names are copied into the config by hand. Without these checks, a copy that is
        // missing would leave the real names in the config, and the tests would write to the real
        // collections, the real graph, the real cache folder, the real decision log and the real
        // content folder.
        assert_eq!(config.falkordb_graph, graph.name());
        assert!(config.items_collection.starts_with(THROWAWAY_PREFIX));
        assert!(
            config
                .concepts_collection
                .starts_with(THROWAWAY_CONCEPTS_PREFIX)
        );
        assert!(config.concept_cache_folder.starts_with(temporary.path()));
        assert!(config.concept_decision_log.starts_with(temporary.path()));
        assert!(config.content_folder.starts_with(temporary.path()));
        Self {
            config,
            temporary,
            _graph: graph,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The three stores, opened with this config.
    pub async fn connect(&self) -> Stores<FalkorGraph> {
        Stores {
            items: ItemStore::connect(&self.config).expect("the item store should open"),
            graph: FalkorGraph::connect(&self.config)
                .await
                .expect("FalkorDB should answer"),
            concepts: ConceptStore::connect(&self.config).expect("the concept store should open"),
        }
    }

    /// Both models of an ingest: an embedder that makes up vectors, and the concept extractor
    /// with this language model and the cache folder and decision log of this config. A test
    /// builds its models here and nowhere else, so it never names those paths by hand.
    pub fn models<L: Llm>(&self, llm: L) -> Models<StandInEmbedder, L> {
        self.models_with_embedder(StandInEmbedder::default(), llm)
    }

    /// Like [`ThrowawayStores::models`], with an embedder that the test has set up.
    pub fn models_with_embedder<L: Llm>(
        &self,
        embedder: StandInEmbedder,
        llm: L,
    ) -> Models<StandInEmbedder, L> {
        Models {
            embedder,
            concepts: ConceptExtractor::new(llm, &self.config),
        }
    }

    /// Runs the `rag-ingest` command against these stores and no others, with the stand-in
    /// `claude` that finds no concept. Every test that runs the command to ingest or to delete
    /// goes through here.
    pub fn rag_ingest(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output {
        self.run_rag_ingest(arguments, ClaudeProgram::StandIn)
    }

    /// Like [`ThrowawayStores::rag_ingest`], but the command starts the real `claude`, so it
    /// spends usage of the subscription. Only the live tests of concepts call it.
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
            (
                "QDRANT_CONCEPTS_COLLECTION",
                self.config.concepts_collection.as_str(),
            ),
            ("FALKORDB_GRAPH", self.config.falkordb_graph.as_str()),
            (
                "CONCEPT_CACHE_DIR",
                path_text(&self.config.concept_cache_folder),
            ),
            (
                "CONCEPT_DECISION_LOG",
                path_text(&self.config.concept_decision_log),
            ),
            ("CONTENT_DIR", path_text(&self.config.content_folder)),
        ];
        // The setting names are typed by hand. A mistyped name would make the command fall back
        // to the real collections, the real graph, the real cache folder, the real decision log
        // and the real content folder, so read the names back the way the command reads them
        // before it starts.
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
        assert_eq!(
            read_back.concepts_collection,
            self.config.concepts_collection
        );
        assert_eq!(read_back.falkordb_graph, self.config.falkordb_graph);
        assert_eq!(
            read_back.concept_cache_folder,
            self.config.concept_cache_folder
        );
        assert_eq!(
            read_back.concept_decision_log,
            self.config.concept_decision_log
        );
        assert_eq!(read_back.content_folder, self.config.content_folder);
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
        // Only a name with the prefix of a throwaway collection is ever deleted.
        let collections: Vec<String> = [
            (&self.config.items_collection, THROWAWAY_PREFIX),
            (&self.config.concepts_collection, THROWAWAY_CONCEPTS_PREFIX),
        ]
        .into_iter()
        .filter(|(name, prefix)| name.starts_with(prefix))
        .map(|(name, _)| name.clone())
        .collect();
        let url = self.config.qdrant_url.clone();
        let names = collections.join(", ");
        // A new thread with its own runtime, because a drop cannot wait inside the runtime of
        // the test.
        let removed = std::thread::spawn(move || -> anyhow::Result<()> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let client = Qdrant::from_url(&url).skip_compatibility_check().build()?;
                let mut result = Ok(());
                for collection in &collections {
                    if let Err(error) = client.delete_collection(collection.as_str()).await {
                        result = Err(error);
                    }
                }
                Ok(result?)
            })
        })
        .join();
        match removed {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!("could not remove the throwaway collections {names}: {error:#}");
            }
            Err(_) => eprintln!(
                "could not remove the throwaway collections {names}: the thread that removes them panicked"
            ),
        }
    }
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a throwaway path should be UTF-8")
}

/// Every point of the items collection the config names, as its identifier and its payload.
pub async fn points_in(config: &Config) -> Vec<(String, Value)> {
    points_of(config, &config.items_collection).await
}

/// Every point of the concepts collection the config names, as its identifier and its payload.
pub async fn concept_points_in(config: &Config) -> Vec<(String, Value)> {
    points_of(config, &config.concepts_collection).await
}

async fn points_of(config: &Config, collection: &str) -> Vec<(String, Value)> {
    let client = Qdrant::from_url(&config.qdrant_url)
        .skip_compatibility_check()
        .build()
        .unwrap();
    let reply = client
        .scroll(
            ScrollPointsBuilder::new(collection)
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
