//! Shared by the tests of this crate: the samples, the command that runs `rag-ingest` against
//! throwaway stores, reads of the stores and the decision log, and the picture stand-in.

mod decisions;
mod stand_in_image_services;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use graph::testing::{GraphSize, StoredDocument, StoredItem, size, stored_document};
use graph::{FalkorGraph, GraphStore, MediaNode};
use qdrant_client::qdrant::ScrollPointsBuilder;
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::{Payload, Qdrant};
use rag_core::{Config, MediaLabels, Tag};
use rag_ingestion::Item;
use serde_json::{Value, json};

pub use decisions::{decision_for_mention, decisions_in, mentions_of};
pub use rag_ingestion::testing::{
    StandInEmbedder, StandInLlm, ThrowawayStores, chapter_at, first_axis, vector_at,
};
pub use stand_in_image_services::{CAPTION, LABEL, StandInImageServices};

pub const SAMPLE_BOOK: &str = "Option Volatility and Pricing";

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

pub trait RunRagIngest {
    /// Runs the `rag-ingest` command against these stores and no others, with the stand-in
    /// `claude` that finds no concept.
    fn rag_ingest(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output;

    /// Like [`RunRagIngest::rag_ingest`], with keys that no service takes. The real keys of a
    /// `.env` above the test would otherwise be read, and a run that went past the check it is
    /// meant to stop at could pay for a page.
    fn rag_ingest_with_fake_keys(
        &self,
        arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Output;
}

impl RunRagIngest for ThrowawayStores {
    fn rag_ingest(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output {
        output_of(rag_ingest_command(self, arguments))
    }

    fn rag_ingest_with_fake_keys(
        &self,
        arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Output {
        let mut command = rag_ingest_command(self, arguments);
        command
            .env("EMBEDDING_GEMINI_API_KEY", "not-a-key")
            .env("CONVERTER_JEV_API_KEY", "not-a-key");
        output_of(command)
    }
}

fn rag_ingest_command(
    stores: &ThrowawayStores,
    arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rag-ingest"));
    command.args(arguments).envs(stores.command_settings());
    put_stand_in_claude_first(&mut command, stores);
    command
}

fn output_of(mut command: Command) -> Output {
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

/// The size of a graph that holds one document of one media and these many items: the media, the
/// document and its items are the nodes, and the edges are one `HAS_ITEM` for each item and one
/// `NEXT` between each two items that follow each other. The media has no edge.
pub fn size_of_one_document(item_count: usize) -> GraphSize {
    let items = item_count as u64;
    GraphSize {
        nodes: items + 2,
        edges: items + items.saturating_sub(1),
    }
}

/// It does not look at the rest of the graph. Returns what the graph holds for the document.
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

/// Returns what the graph holds for the document.
pub async fn assert_graph_holds_only(graph: &FalkorGraph, items: &[Item]) -> StoredDocument {
    let stored = assert_document_stored(graph, items).await;
    assert_eq!(size(graph).await, size_of_one_document(items.len()));
    stored
}

/// Checks that the graph holds the media of the sample chapter with these labels, and that every
/// point of the collection and the one document node of the graph carry a copy of them and these
/// own tags. A label that is empty is left out of a point. Returns the points.
pub async fn assert_labelled(
    config: &Config,
    graph: &FalkorGraph,
    media: &MediaLabels,
    tags: &[&str],
) -> BTreeMap<String, Value> {
    let media_tags: Vec<&str> = media.tags.iter().map(Tag::as_str).collect();
    let points = points_in(config).await;
    for (id, payload) in &points {
        assert_eq!(payload["media"], SAMPLE_BOOK, "{id}");
        assert_eq!(payload["category"], media.category.as_str(), "{id}");
        assert_eq!(payload["authors"], json!(media.authors), "{id}");
        assert_eq!(
            payload.get("media_tags"),
            list_or_nothing(&media_tags).as_ref(),
            "{id}"
        );
        assert_eq!(payload.get("tags"), list_or_nothing(tags).as_ref(), "{id}");
    }
    assert_eq!(
        graph.media().await.unwrap(),
        vec![MediaNode {
            title: SAMPLE_BOOK.to_owned(),
            labels: media.clone(),
        }]
    );
    let nodes = graph.documents().await.unwrap();
    assert_eq!(nodes.len(), 1);
    let labels = &nodes[0].labels;
    assert_eq!(labels.media.as_deref(), Some(SAMPLE_BOOK));
    assert_eq!(labels.category, Some(media.category));
    assert_eq!(labels.authors, media.authors);
    assert_eq!(labels.media_tags, media.tags);
    assert_eq!(
        labels.tags.iter().map(Tag::as_str).collect::<Vec<_>>(),
        tags
    );
    points.into_iter().collect()
}

fn list_or_nothing(texts: &[&str]) -> Option<Value> {
    (!texts.is_empty()).then(|| json!(texts))
}
