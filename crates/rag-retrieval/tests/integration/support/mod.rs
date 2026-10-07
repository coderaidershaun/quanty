//! Shared by the tests of this crate: where the committed chapters are, the items they make, an
//! embedder that reads words, stores filled by hand, and the command that runs `rag-query`
//! against throwaway stores.

mod fixture;
mod word_embedder;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ocr::read_chapter;
use rag_core::{ConceptStore, DocumentInput, Embedder, ItemKind, ItemPoint, ItemStore};
use rag_ingestion::testing::ThrowawayStores;
use rag_ingestion::{Item, chapter_items};

pub use fixture::{Fixture, Placed, QUESTION};
pub use word_embedder::WordEmbedder;

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
pub fn intuition_chapter() -> PathBuf {
    content_folder().join("quanty-sample-notes/chapter-1")
}

/// Written by hand: the derivation and the formulas of the Black–Scholes model.
pub fn in_depth_chapter() -> PathBuf {
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

/// Puts every item of the three committed chapters into the item collection, with the vectors of
/// `embedder`, and returns the items it stored. The concept collection is created too and left
/// empty, because a search reads it.
pub async fn store_samples(
    embedder: &impl Embedder,
    store: &ItemStore,
    concepts: &ConceptStore,
) -> Vec<Item> {
    store
        .ensure_collection()
        .await
        .expect("the collection should be created");
    concepts
        .ensure_collection()
        .await
        .expect("the concept collection should be created");
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

/// Runs the real `rag-query` command from the workspace root against these stores and no others,
/// and prints what it answered, so that one paid run shows everything.
pub fn rag_query(stores: &ThrowawayStores, arguments: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_rag-query"))
        .args(arguments)
        .current_dir(workspace_root())
        .envs(stores.command_settings())
        .output()
        .expect("the rag-query binary should start");
    println!("--- rag-query {arguments:?}: {}", output.status);
    println!("{}", String::from_utf8_lossy(&output.stdout));
    println!("--- its standard error:");
    println!("{}", String::from_utf8_lossy(&output.stderr));
    output
}
