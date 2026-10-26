//! Settings whose stores cannot be reached, and the throwaway stores that hold the committed
//! sample chapters.

use std::collections::BTreeSet;
use std::path::Path;

use mcp::QuantyServer;
use rag_core::{ApiKey, Config, ItemKind, MediaLabels};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores, first_axis};
use rag_ingestion::{ChapterFolder, MediaChange, chapter_items, ingest_chapter, relabel_media};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::{StandInServices, sample_chapter_folder, sample_content_folder};

/// Settings whose Qdrant and FalkorDB addresses nothing listens on, so a store is always down.
/// Every folder is under a temporary folder that goes away with this value.
pub struct ClosedPorts {
    pub config: Config,
    folder: TempDir,
}

impl ClosedPorts {
    pub fn new() -> ClosedPorts {
        let folder = tempfile::tempdir().expect("a temporary folder should be made");
        let config = Config {
            qdrant_url: "http://127.0.0.1:1".to_owned(),
            qdrant_api_key: None,
            falkordb_url: "falkor://127.0.0.1:1".to_owned(),
            falkordb_password: None,
            falkordb_graph: "closed-ports".to_owned(),
            items_collection: "closed-ports-items".to_owned(),
            concepts_collection: "closed-ports-concepts".to_owned(),
            concept_cache_folder: folder.path().join("cache"),
            concept_decision_log: folder.path().join("decisions.jsonl"),
            content_folder: folder.path().join("content"),
            dataset_folder: folder.path().join("datasets"),
            dataset_cache_folder: folder.path().join("dataset-cache"),
            gemini_api_key: Some(ApiKey::new("a-made-up-key")),
            jev_api_key: None,
        };
        ClosedPorts { config, folder }
    }

    pub fn with_content_folder(mut self, content_folder: &Path) -> ClosedPorts {
        self.config.content_folder = content_folder.to_path_buf();
        self
    }

    pub fn folder(&self) -> &Path {
        self.folder.path()
    }

    pub fn server(&self) -> QuantyServer<StandInServices> {
        QuantyServer::new(self.config.clone(), StandInServices::new())
    }

    pub fn server_with_limit(&self, max_pdf_bytes: u64) -> QuantyServer<StandInServices> {
        QuantyServer::with_max_pdf_bytes(self.config.clone(), StandInServices::new(), max_pdf_bytes)
    }
}

/// The question that the throwaway library is built to answer first with the figure of page 5.
pub const QUESTION: &str = "What do the three spreads of Figure 13-4 have in common?";

/// The throwaway stores that hold the three committed chapters, ingested with stand-ins that
/// place the figure of page 5 of the sample chapter at the question. The media of the sample
/// chapter has an author and a tag.
pub struct Library {
    pub stores: ThrowawayStores,
    /// What the server asks of the model. The ingest asked it too.
    pub llm: StandInLlm,
    figure_text: String,
}

pub const AUTHOR: &str = "Sheldon Natenberg";

impl Library {
    /// The concept "Long straddle" is found in every item that mentions a straddle, which only page
    /// 5 of the sample chapter does.
    pub async fn ingested_with_concepts(test_name: &str) -> Library {
        Library::ingested_finding(test_name, |text| {
            let concepts = if text.contains("straddle") {
                json!([{ "name": "Long straddle", "definition": "A bet that the price moves a lot, either way." }])
            } else {
                json!([])
            };
            json!({ "concepts": concepts, "relations": [] })
        })
        .await
    }

    /// `concepts_of(text)` is what the stand-in model replies to the question about the concepts of
    /// an item with this text.
    async fn ingested_finding(
        test_name: &str,
        concepts_of: impl Fn(&str) -> Value + Send + Sync + 'static,
    ) -> Library {
        let stores = ThrowawayStores::new(test_name);
        let figure_text = chapter_items(
            &ocr::read_chapter(&sample_chapter_folder()).expect("the sample chapter should read"),
        )
        .into_iter()
        .find(|item| item.payload.kind == ItemKind::Figure && item.payload.page == 5)
        .expect("page 5 of the sample chapter has a figure")
        .input
        .text;
        let library = Library {
            stores,
            llm: StandInLlm::replying_to_questions(move |question, _| {
                Ok(if question.input.starts_with("Question: ") {
                    written_answer()
                } else {
                    concepts_of(&question.input)
                })
            }),
            figure_text,
        };
        let connected = library.stores.connect().await;
        let models = library
            .stores
            .models_with_embedder(library.embedder(), library.llm.clone());
        for chapter in [
            sample_chapter_folder(),
            sample_content_folder().join("quanty-sample-notes/chapter-1"),
            sample_content_folder().join("quanty-sample-notes/chapter-2"),
        ] {
            let folder = ChapterFolder {
                folder: &chapter,
                new_media: &MediaLabels::default(),
            };
            ingest_chapter(folder, &models, &connected)
                .await
                .expect("a sample chapter should be ingested");
        }
        let labels = MediaChange {
            authors: Some(vec![AUTHOR.to_owned()]),
            tags: Some(BTreeSet::from(["options".parse().expect("a tag")])),
            ..MediaChange::default()
        };
        relabel_media("Option Volatility and Pricing", &labels, &connected)
            .await
            .expect("the media of the sample chapter should be labelled");
        library
    }

    fn embedder(&self) -> StandInEmbedder {
        StandInEmbedder::default()
            .placing(QUESTION, first_axis())
            .placing(&self.figure_text, first_axis())
    }

    /// A server over these stores that reads and writes the chapters in `content_folder`, with
    /// these services. A test that writes into the chapters gives a copy of the samples, which
    /// holds the same chapters that the stores were ingested from.
    pub fn server_over(
        &self,
        content_folder: &Path,
        services: StandInServices,
    ) -> QuantyServer<StandInServices> {
        let config = Config {
            content_folder: content_folder.to_path_buf(),
            ..self.stores.config().clone()
        };
        QuantyServer::new(config, services)
    }

    pub fn server(&self) -> QuantyServer<StandInServices> {
        let config = Config {
            content_folder: sample_content_folder(),
            ..self.stores.config().clone()
        };
        let figure_text = self.figure_text.clone();
        let services = StandInServices::new()
            .with_embedder(move || {
                StandInEmbedder::default()
                    .placing(QUESTION, first_axis())
                    .placing(&figure_text, first_axis())
            })
            .with_llm(self.llm.clone());
        QuantyServer::new(config, services)
    }
}

/// What the stand-in model replies to a question that asks for an answer: two claims, the second
/// of which rests on the first result twice over and on the second result.
fn written_answer() -> Value {
    json!({
        "title": "What the three spreads share",
        "claims": [
            { "heading": "The chart", "text": "At 48.40 the three spreads show the same theoretical profit.", "sources": [1] },
            { "text": "A short straddle, a ratio spread and a long butterfly all start there.", "sources": [1, 2, 1] },
        ],
        "follow_ups": ["What is a long butterfly?", "  ", "What is a long butterfly?"],
    })
}
