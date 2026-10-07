//! What the tests of the live backend share: stand-in services over throwaway stores, with the
//! paid page services stubbed, and a config whose stores cannot be reached.
// No test uses these yet. This is an `allow` and not an `expect`, so the day a test starts to
// use them does not make the build fail.
#![allow(dead_code)]

use std::sync::Arc;

use graph::FalkorGraph;
use gui::backend::live::{LiveContext, Services};
use gui::contract::Failure;
use ocr::ConvertError;
use ocr::testing::{Scenario, StubServices};
use rag_core::{ApiKey, Config};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores};

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
