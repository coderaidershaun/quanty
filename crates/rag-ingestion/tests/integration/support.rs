//! Shared by the tests of this crate: where the committed chapters are, an embedder that makes
//! up vectors, and a collection that is removed when the test ends.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use qdrant_client::qdrant::ScrollPointsBuilder;
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::{Payload, Qdrant};
use rag_core::{Config, DocumentInput, EMBEDDING_DIMENSIONS, EmbedError, Embedder, Embedding};
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

/// A config that stores into a collection no one else uses. The collection is deleted when this
/// is dropped, so it goes away also when the test fails. It never names the real collection.
pub struct ThrowawayCollection {
    config: Config,
}

impl ThrowawayCollection {
    pub fn new(test_name: &str) -> Self {
        let nanoseconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let name = format!(
            "{THROWAWAY_PREFIX}{test_name}-{}-{nanoseconds}",
            std::process::id()
        );
        let config = Config {
            items_collection: name,
            ..Config::load().expect("the settings should load")
        };
        Self { config }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }
}

impl Drop for ThrowawayCollection {
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
