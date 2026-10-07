//! Runs whole chapters through ingestion with a language model that answers by a rule, into a
//! throwaway collection of the local Qdrant and a throwaway graph of the local FalkorDB, so
//! nothing is billed. It shows what is asked, what is written, what is kept, and what happens when
//! the model fails.

mod cache;
mod extraction;
mod failures;
mod resolution;
mod script;

use std::collections::HashMap;
use std::path::Path;

use ocr::read_chapter;
use rag_ingestion::{Item, chapter_items};
use serde_json::{Value, json};

/// What the model is asked, as the stand-in sees it. It is also what is embedded for the item,
/// so this pins what is sent.
fn input_of(item: &Item) -> String {
    format!("{}\n\n{}", item.input.title, item.input.text)
}

fn items_of(chapter_folder: &Path) -> Vec<Item> {
    chapter_items(&read_chapter(chapter_folder).unwrap())
}

/// The position in reading order of the item that a question is about.
fn positions_of(items: &[Item]) -> HashMap<String, usize> {
    items
        .iter()
        .enumerate()
        .map(|(position, item)| (input_of(item), position))
        .collect()
}

fn finding_volatility() -> Value {
    json!({
        "concepts": [{ "name": "volatility", "definition": "how much a price moves over time" }],
        "relations": [],
    })
}

fn files_in(folder: &Path) -> usize {
    std::fs::read_dir(folder).unwrap().count()
}
