//! Stand-in services for the server under test, so that no call reaches a paid service.

use std::sync::Arc;

use mcp::Services;
use ocr::convert::convert_chapter_with;
use ocr::testing::{Scenario, StubServices};
use ocr::{ChapterJob, ConversionSummary, ConvertError};
use rag_core::{
    Config, EmbedError, Llm, LlmError, LlmReply, ModelSettings, Question, SettingsError, WorkKind,
};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm};
use rag_ingestion::{KeyState, KeyStates};
use tokio::sync::{Mutex, mpsc};

/// Services that never reach a paid model. `convert` waits at the gate when a test set one, and
/// so does the free sign-in check of the model when a test set the ready gate.
pub struct StandInServices {
    embedder: Box<dyn Fn() -> Result<StandInEmbedder, EmbedError> + Send + Sync>,
    key_states: Option<KeyStates>,
    llm: StandInLlm,
    pages: StubServices,
    gate: Option<Mutex<mpsc::UnboundedReceiver<()>>>,
    ready_gate: Option<Arc<GateSide>>,
    question_gate: Option<Arc<Mutex<mpsc::UnboundedReceiver<()>>>>,
}

impl StandInServices {
    pub fn new() -> StandInServices {
        StandInServices {
            embedder: Box::new(|| Ok(StandInEmbedder::default())),
            key_states: None,
            llm: StandInLlm::finding_nothing(),
            pages: StubServices::new(Scenario::SampleChapter),
            gate: None,
            ready_gate: None,
            question_gate: None,
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
        self.embedder = Box::new(move || Ok(embedder()));
        self
    }

    /// Services whose embedder cannot be made: it fails with `error`.
    pub fn with_failing_embedder(
        mut self,
        error: impl Fn() -> EmbedError + Send + Sync + 'static,
    ) -> StandInServices {
        self.embedder = Box::new(move || Err(error()));
        self
    }

    /// The states that the key checks find, whatever the settings hold. Without them a key that
    /// the settings hold is accepted and one that they lack is missing.
    pub fn with_key_states(mut self, key_states: KeyStates) -> StandInServices {
        self.key_states = Some(key_states);
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

    /// The same services, and the gate that holds each free sign-in check of the model until the
    /// test lets it go on. A build checks the model before its first step, so this holds a build
    /// after its job is listed.
    pub fn with_ready_gate(self) -> (StandInServices, ReadyGate) {
        let (reached, test_reached) = mpsc::unbounded_channel();
        let (open, gate) = mpsc::unbounded_channel();
        let side = GateSide {
            reached,
            open: Mutex::new(gate),
        };
        (
            StandInServices {
                ready_gate: Some(Arc::new(side)),
                ..self
            },
            ReadyGate {
                reached: test_reached,
                open,
            },
        )
    }

    /// The same services, and the sender that lets one question to the model go on for each `()`
    /// it sends. Until then that question waits, so a job that asks stays in the middle of it.
    pub fn with_question_gate(self) -> (StandInServices, mpsc::UnboundedSender<()>) {
        let (open, gate) = mpsc::unbounded_channel();
        (
            StandInServices {
                question_gate: Some(Arc::new(Mutex::new(gate))),
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

/// What a test holds of the ready gate. `reached` gets one `()` for each sign-in check that came
/// to the gate, and each `()` sent on `open` lets one check go on. A dropped `open` opens the
/// gate, so a test that fails does not hang.
pub struct ReadyGate {
    pub reached: mpsc::UnboundedReceiver<()>,
    pub open: mpsc::UnboundedSender<()>,
}

/// The model's side of the ready gate.
struct GateSide {
    reached: mpsc::UnboundedSender<()>,
    open: Mutex<mpsc::UnboundedReceiver<()>>,
}

/// The stand-in model of `StandInServices`. Its free sign-in check waits at the ready gate when a
/// test set one; everything else is the stand-in's.
#[derive(Clone)]
pub struct GatedLlm {
    llm: StandInLlm,
    gate: Option<Arc<GateSide>>,
    question_gate: Option<Arc<Mutex<mpsc::UnboundedReceiver<()>>>>,
}

impl Llm for GatedLlm {
    fn model(&self) -> &str {
        self.llm.model()
    }

    fn cache_identity(&self) -> String {
        self.llm.cache_identity()
    }

    async fn check_ready(&self) -> Result<(), LlmError> {
        if let Some(gate) = &self.gate {
            // A test that does not count the checks may have dropped `reached`.
            let _ = gate.reached.send(());
            gate.open.lock().await.recv().await;
        }
        self.llm.check_ready().await
    }

    async fn ask(&self, question: Question<'_>) -> Result<LlmReply, LlmError> {
        if let Some(gate) = &self.question_gate {
            // A dropped sender opens the gate too, so a test that fails does not hang.
            gate.lock().await.recv().await;
        }
        self.llm.ask(question).await
    }
}

impl Services for StandInServices {
    type Embedder = StandInEmbedder;
    type Llm = GatedLlm;

    fn embedder(&self, _config: &Config) -> Result<StandInEmbedder, EmbedError> {
        (self.embedder)()
    }

    async fn check_keys(&self, config: &Config) -> KeyStates {
        self.key_states.unwrap_or_else(|| {
            let state = |held: bool| {
                if held {
                    KeyState::Accepted
                } else {
                    KeyState::Missing
                }
            };
            KeyStates {
                gemini: state(config.gemini_api_key.is_some()),
                jev: state(config.jev_api_key.is_some()),
            }
        })
    }

    fn model_choice(&self, _work: WorkKind) -> Result<ModelSettings, SettingsError> {
        Ok(ModelSettings {
            provider: "stand-in".to_owned(),
            model: "stand-in".to_owned(),
            thinking: "medium".to_owned(),
        })
    }

    fn llm(&self, _work: WorkKind) -> GatedLlm {
        GatedLlm {
            llm: self.llm.clone(),
            gate: self.ready_gate.clone(),
            question_gate: self.question_gate.clone(),
        }
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
