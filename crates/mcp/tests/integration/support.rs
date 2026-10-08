//! What the tests share: stand-in services, settings whose stores cannot be reached, and an
//! in-process MCP client.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mcp::{QuantyServer, Services};
use ocr::convert::convert_chapter_with;
use ocr::testing::{Scenario, StubServices};
use ocr::{ChapterJob, ConversionSummary, ConvertError};
use rag_core::{ApiKey, Config, DocId, EmbedError, ItemKind, MediaLabels};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores, first_axis};
use rag_ingestion::{ChapterFolder, MediaChange, chapter_items, ingest_chapter, relabel_media};
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Map, Value, json};
use tempfile::TempDir;
use tokio::sync::{Mutex, mpsc};

/// Services that never reach a paid model. `convert` waits at the gate when a test set one.
pub struct StandInServices {
    embedder: Box<dyn Fn() -> StandInEmbedder + Send + Sync>,
    llm: StandInLlm,
    pages: StubServices,
    gate: Option<Mutex<mpsc::UnboundedReceiver<()>>>,
}

impl StandInServices {
    pub fn new() -> StandInServices {
        StandInServices {
            embedder: Box::new(StandInEmbedder::default),
            llm: StandInLlm::finding_nothing(),
            pages: StubServices::new(Scenario::SampleChapter),
            gate: None,
        }
    }

    pub fn with_pages(mut self, scenario: Scenario) -> StandInServices {
        self.pages = StubServices::new(scenario);
        self
    }

    pub fn with_embedder(
        mut self,
        embedder: impl Fn() -> StandInEmbedder + Send + Sync + 'static,
    ) -> StandInServices {
        self.embedder = Box::new(embedder);
        self
    }

    /// The same services, and the sender that lets one `convert` go on for each `()` it sends.
    /// Until then that `convert` waits, so its job stays in the stage `converting`.
    pub fn with_gate(self) -> (StandInServices, mpsc::UnboundedSender<()>) {
        let (open, gate) = mpsc::unbounded_channel();
        (
            StandInServices {
                gate: Some(Mutex::new(gate)),
                ..self
            },
            open,
        )
    }

    /// A clone of `llm` keeps what the server asks of it.
    pub fn with_llm(mut self, llm: StandInLlm) -> StandInServices {
        self.llm = llm;
        self
    }
}

impl Services for StandInServices {
    type Embedder = StandInEmbedder;
    type Llm = StandInLlm;

    fn embedder(&self, _config: &Config) -> Result<StandInEmbedder, EmbedError> {
        Ok((self.embedder)())
    }

    fn llm(&self, _model: &str) -> StandInLlm {
        self.llm.clone()
    }

    async fn convert(
        &self,
        job: &ChapterJob,
        _config: &Config,
    ) -> Result<ConversionSummary, ConvertError> {
        if let Some(gate) = &self.gate {
            // A dropped sender opens the gate too, so a test that fails does not hang.
            gate.lock().await.recv().await;
        }
        convert_chapter_with(job, &self.pages).await
    }
}

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
            falkordb_url: "falkor://127.0.0.1:1".to_owned(),
            falkordb_graph: "closed-ports".to_owned(),
            items_collection: "closed-ports-items".to_owned(),
            concepts_collection: "closed-ports-concepts".to_owned(),
            concept_cache_folder: folder.path().join("cache"),
            concept_decision_log: folder.path().join("decisions.jsonl"),
            content_folder: folder.path().join("content"),
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

/// The committed sample files of the workspace. Tests only read them.
pub fn samples_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples")
}

pub fn sample_content_folder() -> PathBuf {
    samples_folder().join("content")
}

/// The chapter that the sample PDF is converted to, which has seven pages.
pub fn sample_chapter_folder() -> PathBuf {
    sample_content_folder().join("option-volatility-and-pricing/chapter-1")
}

pub fn sample_document_id() -> DocId {
    let index = ocr::ChapterIndex::read(&sample_chapter_folder())
        .expect("the sample chapter should have a chapter.json");
    DocId::from_source_sha256(&index.source_sha256)
}

pub type Client = RunningService<RoleClient, ()>;

pub async fn connect<S: Services>(server: QuantyServer<S>) -> Client {
    let (server_end, client_end) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let running = server
            .serve(server_end)
            .await
            .expect("the server should start");
        running
            .waiting()
            .await
            .expect("the server should end without a panic");
    });
    ().serve(client_end)
        .await
        .expect("the client should connect")
}

/// Calls a tool and gives back the whole result, an error result too.
pub async fn call(client: &Client, tool: &'static str, arguments: Value) -> CallToolResult {
    let arguments: Map<String, Value> = match arguments {
        Value::Object(map) => map,
        other => panic!("the arguments of a tool call are a JSON object, not {other}"),
    };
    client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await
        .unwrap_or_else(|error| panic!("the call of {tool} should get a result: {error}"))
}

pub fn structured(result: &CallToolResult) -> &Value {
    assert_ne!(
        result.is_error,
        Some(true),
        "the call failed: {}",
        text_of(result)
    );
    result
        .structured_content
        .as_ref()
        .expect("a tool result should be structured")
}

/// The text of a call that failed, which is a tool error and not a protocol error.
pub fn error_text(result: &CallToolResult) -> String {
    assert_eq!(
        result.is_error,
        Some(true),
        "the call should have failed, but gave {:?}",
        result.structured_content
    );
    text_of(result)
}

fn text_of(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| block.as_text())
        .map(|text| text.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
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
    pub async fn ingested(test_name: &str) -> Library {
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
            llm: StandInLlm::replying_to_questions(|question, _| {
                Ok(if question.input.starts_with("Question: ") {
                    written_answer()
                } else {
                    json!({ "concepts": [], "relations": [] })
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

/// How long a job may take to end in a test: seven pages of stand-in calls and an ingest.
const JOB_WAIT: Duration = Duration::from_secs(180);

pub async fn report_when_ended(client: &Client, job_id: &str) -> Value {
    let started = Instant::now();
    loop {
        let result = call(client, "ingest_status", json!({ "job_id": job_id })).await;
        let report = structured(&result).clone();
        if report["state"] != "running" {
            return report;
        }
        assert!(
            started.elapsed() < JOB_WAIT,
            "the job did not end in time: {report}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
