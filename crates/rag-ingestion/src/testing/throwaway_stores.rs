//! Stores and folders that no one else uses and that are removed when the test ends: two
//! collections of the local Qdrant, a graph of the local FalkorDB, and a temporary folder.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use graph::FalkorGraph;
use graph::testing::ThrowawayGraph;
use qdrant_client::Qdrant;
use rag_core::{ConceptStore, Config, ItemStore, Llm};
use tempfile::TempDir;

use super::StandInEmbedder;
use crate::{ConceptExtractor, Models, Stores};

const THROWAWAY_PREFIX: &str = "test-items-";
const THROWAWAY_CONCEPTS_PREFIX: &str = "test-concepts-";

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

impl ThrowawayStores {
    pub fn new(test_name: &str) -> Self {
        let nanoseconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock should be after 1970")
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
            // The real key would be copied by `..settings`, and no test may hand it to the
            // converter, which bills for every call it makes.
            jev_api_key: None,
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

    /// The temporary folder that goes away with the stores. A test puts what it needs next to the
    /// stores in it.
    pub fn temporary_folder(&self) -> &Path {
        self.temporary.path()
    }

    /// The settings that a command of this project is started with so that it reaches these stores
    /// and no others: each name with its value. Every test that starts a binary of this project
    /// passes exactly this list to it.
    pub fn command_settings(&self) -> Vec<(&'static str, String)> {
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
        settings
            .into_iter()
            .map(|(name, value)| (name, value.to_owned()))
            .collect()
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
