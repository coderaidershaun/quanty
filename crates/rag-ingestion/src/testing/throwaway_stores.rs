//! Stores and folders that no one else uses and that are removed when the test ends: two
//! collections of the local Qdrant, a graph of the local FalkorDB, and a temporary folder.

use std::path::Path;

use graph::FalkorGraph;
use graph::testing::{ThrowawayGraph, remove_on_thread, unique_name};
use qdrant_client::Qdrant;
use rag_core::{ConceptStore, Config, ItemStore, Llm};
use tempfile::TempDir;

use super::StandInEmbedder;
use crate::{ConceptExtractor, InsightsWriter, Models, Stores};

const THROWAWAY_PREFIX: &str = "test-items-";
const THROWAWAY_CONCEPTS_PREFIX: &str = "test-concepts-";

/// A config that stores into two collections, a graph, a cache folder, a decision log, a content
/// folder, a dataset folder and a dataset cache folder that no one else uses. The collections and
/// the graph are deleted when this is dropped, and the folders and the log go with a temporary
/// folder, so they go away also when the test fails. It never names a real collection, the real
/// graph, the real cache folder, the real decision log, the real content folder or the real
/// dataset folders, and every store a test touches is reached through this config and from
/// nowhere else.
pub struct ThrowawayStores {
    config: Config,
    temporary: TempDir,
    _graph: ThrowawayGraph,
}

impl ThrowawayStores {
    pub fn new(test_name: &str) -> Self {
        let stamp = unique_name(test_name);
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
            dataset_folder: temporary.path().join("datasets"),
            dataset_cache_folder: temporary.path().join("dataset-cache"),
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
        assert!(config.dataset_folder.starts_with(temporary.path()));
        assert!(config.dataset_cache_folder.starts_with(temporary.path()));
        Self {
            config,
            temporary,
            _graph: graph,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub async fn connect(&self) -> Stores {
        Stores {
            items: ItemStore::connect(&self.config).expect("the item store should open"),
            graph: FalkorGraph::connect(&self.config)
                .await
                .expect("FalkorDB should answer"),
            concepts: ConceptStore::connect(&self.config).expect("the concept store should open"),
        }
    }

    /// Uses these isolated stores, a stand-in embedder and the supplied model for artifacts and concepts.
    /// Insights are disabled, but required artifact checkpoints can still be written to the chapter folder.
    /// A test builds its models here and nowhere else, so it never names those paths by hand.
    pub fn models<L: Llm + Clone>(&self, llm: L) -> Models<StandInEmbedder, L> {
        self.models_with_embedder(StandInEmbedder::default(), llm)
    }

    pub fn models_with_embedder<L: Llm + Clone>(
        &self,
        embedder: StandInEmbedder,
        llm: L,
    ) -> Models<StandInEmbedder, L> {
        Models {
            embedder,
            artifacts: crate::ArtifactWriter::new(llm.clone()),
            concepts: ConceptExtractor::new(llm, &self.config),
            insights: None,
        }
    }

    /// [`ThrowawayStores::models`] that also writes insights, asking a clone of `llm`. A clone of
    /// a stand-in model shares what it records, so one value counts every kind of question.
    pub fn models_writing_insights<L: Llm + Clone>(&self, llm: L) -> Models<StandInEmbedder, L> {
        Models {
            insights: Some(InsightsWriter::new(llm.clone())),
            ..self.models(llm)
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
            ("DATASET_DIR", path_text(&self.config.dataset_folder)),
            (
                "DATASET_CACHE_DIR",
                path_text(&self.config.dataset_cache_folder),
            ),
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
        assert_eq!(read_back.dataset_folder, self.config.dataset_folder);
        assert_eq!(
            read_back.dataset_cache_folder,
            self.config.dataset_cache_folder
        );
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
        remove_on_thread(
            &format!("the throwaway collections {names}"),
            "them",
            move || -> anyhow::Result<()> {
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
            },
        );
    }
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a throwaway path should be UTF-8")
}
