//! Shared by the tests of this crate: where the committed chapters are, the items they make, an
//! embedder that reads words, and a collection that is removed when the test ends.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use ocr::read_chapter;
use qdrant_client::Qdrant;
use rag_core::{
    Config, DocumentInput, EMBEDDING_DIMENSIONS, EmbedError, Embedder, Embedding, ItemKind,
    ItemPoint, ItemStore,
};
use rag_ingestion::{Item, chapter_items};

const THROWAWAY_PREFIX: &str = "test-items-";

/// The title that every item of the sample chapter of the book carries.
pub const SAMPLE_CHAPTER_TITLE: &str = "Option Volatility and Pricing, chapter 1: Sample Pages";

/// The title that every item of the hand-written chapter on the model in depth carries.
pub const IN_DEPTH_CHAPTER_TITLE: &str = "Quanty Sample Notes, chapter 2: Black Scholes In Depth";

/// The title that every item of the hand-written intuition chapter carries.
pub const INTUITION_CHAPTER_TITLE: &str =
    "Quanty Sample Notes, chapter 1: Options Pricing Intuition";

/// The folder of the workspace, where `golden.toml` is.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn content_folder() -> PathBuf {
    workspace_root().join("samples/content")
}

/// Seven pages converted from a real book: every kind of piece, headings, footnotes and page
/// breaks.
pub fn sample_chapter() -> PathBuf {
    content_folder().join("option-volatility-and-pricing/chapter-1")
}

/// Written by hand: options pricing at the level of intuition.
fn intuition_chapter() -> PathBuf {
    content_folder().join("quanty-sample-notes/chapter-1")
}

/// Written by hand: the derivation and the formulas of the Black–Scholes model.
fn in_depth_chapter() -> PathBuf {
    content_folder().join("quanty-sample-notes/chapter-2")
}

/// Every item of the three committed chapters, in the order of the chapters. The folders are
/// made absolute first, so a figure's picture path is absolute, as ingestion stores it.
pub fn sample_items() -> Vec<Item> {
    [sample_chapter(), intuition_chapter(), in_depth_chapter()]
        .iter()
        .flat_map(|folder| {
            let folder = std::fs::canonicalize(folder).expect("a sample chapter should exist");
            let chapter = read_chapter(&folder).expect("a sample chapter should be readable");
            chapter_items(&chapter)
        })
        .collect()
}

/// Puts every item of the three committed chapters into the collection of `store`, with the
/// vectors of `embedder`, and returns the items it stored.
pub async fn store_samples(embedder: &impl Embedder, store: &ItemStore) -> Vec<Item> {
    store
        .ensure_collection()
        .await
        .expect("the collection should be created");
    let items = sample_items();
    let inputs: Vec<DocumentInput> = items.iter().map(|item| item.input.clone()).collect();
    let vectors = embedder
        .embed_document(&inputs)
        .await
        .expect("the items should be embedded");
    let points: Vec<ItemPoint> = items
        .iter()
        .zip(vectors)
        .map(|(item, vector)| ItemPoint {
            id: item.id,
            vector,
            payload: item.payload.clone(),
        })
        .collect();
    store
        .upsert(&points)
        .await
        .expect("the points should be stored");
    items
}

/// The first item of that document, kind and page. The test fails when there is none.
pub fn find_item<'a>(items: &'a [Item], title: &str, kind: ItemKind, page: u32) -> &'a Item {
    items
        .iter()
        .find(|item| {
            item.payload.doc_title == title
                && item.payload.kind == kind
                && item.payload.page == page
        })
        .unwrap_or_else(|| panic!("no {} on page {page} of {title}", kind.as_str()))
}

/// Makes a vector from the words of a text, so a question that repeats the words of an item
/// lands near that item. It also keeps every question it is asked.
// SMELL: this is a stand-in that knows nothing of meaning. Every word counts the same and many
// words share one place in the vector, so only a question that is the whole stored text of an
// item is sure to find it. It shows that search is wired up, not that it finds good answers.
#[derive(Clone, Default)]
pub struct WordEmbedder {
    questions: Arc<Mutex<Vec<String>>>,
}

impl WordEmbedder {
    pub fn questions(&self) -> Vec<String> {
        self.questions.lock().unwrap().clone()
    }
}

impl Embedder for WordEmbedder {
    async fn embed_document(&self, inputs: &[DocumentInput]) -> Result<Vec<Embedding>, EmbedError> {
        Ok(inputs
            .iter()
            .map(|input| word_vector(&format!("{} {}", input.title, input.text)))
            .collect())
    }

    async fn embed_query(&self, query: &str) -> Result<Embedding, EmbedError> {
        self.questions.lock().unwrap().push(query.to_owned());
        Ok(word_vector(query))
    }
}

/// One count for each word, in the dimension that the word's hash picks, scaled to length 1.
fn word_vector(text: &str) -> Embedding {
    let mut vector = vec![0.0_f32; EMBEDDING_DIMENSIONS];
    let lowercase = text.to_lowercase();
    let words = lowercase
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty());
    for word in words {
        vector[(fnv1a(word) % EMBEDDING_DIMENSIONS as u64) as usize] += 1.0;
    }
    let norm = vector.iter().map(|count| count * count).sum::<f32>().sqrt();
    if norm == 0.0 {
        vector[0] = 1.0;
        return vector;
    }
    vector.into_iter().map(|count| count / norm).collect()
}

/// Written out here and not taken from the standard library, because the standard hasher may
/// give other numbers in another Rust release, and these tests need the same vectors on every
/// run.
fn fnv1a(word: &str) -> u64 {
    word.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// A config that reads from a collection no one else uses. The collection is deleted when this
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
