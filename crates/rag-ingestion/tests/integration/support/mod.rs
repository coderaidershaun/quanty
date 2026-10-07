//! Shared by the tests of this crate: where the samples are, the command that runs `rag-ingest`
//! against throwaway stores, reads of what the stores hold, the stand-in for the picture call, and
//! reads of the decision log.

mod decisions;
mod stand_in_image_services;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use graph::FalkorGraph;
use graph::testing::{GraphSize, StoredDocument, StoredItem, size, stored_document};
use qdrant_client::qdrant::ScrollPointsBuilder;
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::{Payload, Qdrant};
use rag_core::Config;
use rag_ingestion::Item;
use serde_json::Value;

pub use decisions::{decision_for_mention, decisions_in, mentions_of};
pub use rag_ingestion::testing::{
    StandInEmbedder, StandInLlm, ThrowawayStores, first_axis, vector_at,
};
pub use stand_in_image_services::{CAPTION, LABEL, StandInImageServices};

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

/// The key that the command is given for Gemini. It is not a real key, so nothing is billed, and
/// it only has to exist, so that the command gets as far as the conversion.
const STAND_IN_GEMINI_KEY: &str = "not-a-real-key";

/// Which `claude` program the `rag-ingest` command under test starts.
enum ClaudeProgram {
    /// The committed stand-in, which finds no concept and bills nothing.
    StandIn,
    /// The real one, on the subscription.
    Real,
}

/// Runs the `rag-ingest` command against a set of throwaway stores.
pub trait RunRagIngest {
    /// Runs the `rag-ingest` command against these stores and no others, with the stand-in
    /// `claude` that finds no concept. Every test that runs the command to ingest or to delete
    /// goes through here.
    fn rag_ingest(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output;

    /// Like [`RunRagIngest::rag_ingest`], but the command starts the real `claude`, so it
    /// spends usage of the subscription. Only the live tests of concepts call it.
    fn rag_ingest_asking_claude(
        &self,
        arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Output;

    /// Runs `rag-ingest pdf` with these arguments after `pdf`, against these stores and no
    /// others. It is the only way a test starts `pdf` on a real file, and no test starts `pdf`
    /// through [`RunRagIngest::rag_ingest`] or [`RunRagIngest::rag_ingest_asking_claude`]. A real
    /// run of `pdf` converts with `claude` and with Jev, which bill for every call, so the guard
    /// is here and a test cannot leave it out:
    /// - the command runs in a folder with an empty `.env` file, because the search for a `.env`
    ///   stops at the first one it finds, so the `.env` of the workspace with its real keys is
    ///   never read;
    /// - it gets no Jev key, so the conversion stops before its first page, and it gets no
    ///   `ANTHROPIC_API_KEY`;
    /// - the `claude` that it finds first is the stand-in.
    fn rag_ingest_pdf(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output;
}

impl RunRagIngest for ThrowawayStores {
    fn rag_ingest(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output {
        run_rag_ingest(self, arguments, ClaudeProgram::StandIn)
    }

    fn rag_ingest_asking_claude(
        &self,
        arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Output {
        run_rag_ingest(self, arguments, ClaudeProgram::Real)
    }

    fn rag_ingest_pdf(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output {
        run_rag_ingest_pdf(self, arguments)
    }
}

fn run_rag_ingest(
    stores: &ThrowawayStores,
    arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
    claude: ClaudeProgram,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rag-ingest"));
    command.args(arguments).envs(stores.command_settings());
    assert_ne!(
        command.get_args().next(),
        Some(OsStr::new("pdf")),
        "`pdf` converts with paid services, so a test starts it through `rag_ingest_pdf` only"
    );
    if let ClaudeProgram::StandIn = claude {
        put_stand_in_claude_first(&mut command, stores);
    }
    command
        .output()
        .expect("the rag-ingest binary should start")
}

fn run_rag_ingest_pdf(
    stores: &ThrowawayStores,
    arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Output {
    let working_folder = stores.temporary_folder().join("pdf-command");
    fs::create_dir_all(&working_folder).expect("the working folder should be made");
    fs::write(working_folder.join(".env"), "").expect("the empty .env file should be written");
    let mut command = Command::new(env!("CARGO_BIN_EXE_rag-ingest"));
    command
        .arg("pdf")
        .args(arguments)
        .current_dir(&working_folder)
        .envs(stores.command_settings())
        .env_remove("CONVERTER_JEV_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env("EMBEDDING_GEMINI_API_KEY", STAND_IN_GEMINI_KEY);
    put_stand_in_claude_first(&mut command, stores);
    command
        .output()
        .expect("the rag-ingest binary should start")
}

fn put_stand_in_claude_first(command: &mut Command, stores: &ThrowawayStores) {
    let work = stores.temporary_folder().join("stand-in-claude");
    fs::create_dir_all(&work).expect("the stand-in folder should be made");
    fs::write(work.join("stdout"), STAND_IN_ANSWER).expect("the stand-in answer should be written");
    let path = format!(
        "{}:{}",
        stand_in_claude_folder().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    command.env("PATH", path).env("STAND_IN_CLAUDE", &work);
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
