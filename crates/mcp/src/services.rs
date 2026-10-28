//! The outside services that one tool call needs, behind one trait: `main` hands the paid ones to
//! the server and a test hands stand-ins, so no test can reach a paid service.

use std::future::Future;

use ocr::{ChapterJob, ConversionSummary, ConvertError, PageProgress};
use rag_core::{
    AgentCli, ApiKey, Config, EmbedError, Embedder, GeminiEmbedder, Llm, ModelSettings,
    SettingsError, WorkKind,
};
use rag_ingestion::{KeyClients, KeyStates};

/// Makes the outside services of one tool call. The graph is not part of it: a call always opens
/// the FalkorDB graph that its settings name, and a test points those settings at a throwaway one.
pub trait Services: Send + Sync + 'static {
    type Embedder: Embedder + Send + Sync + 'static;
    type Llm: Llm + Send + Sync + 'static;

    /// # Errors
    /// [`EmbedError::MissingApiKey`] when the settings hold no key for the embedder.
    fn embedder(&self, config: &Config) -> Result<Self::Embedder, EmbedError>;

    /// The model client for one kind of work. Real calls load that kind's saved choice.
    fn llm(&self, work: WorkKind) -> Self::Llm;

    /// Converts the pages of the chapter that are not converted yet, and tells each page to
    /// `on_page` as it is cut out and as it ends. This is the part of an ingest that pays for a
    /// model call for every page.
    ///
    /// # Errors
    /// The errors of `ocr::convert_chapter_with_jev_key`.
    fn convert(
        &self,
        job: &ChapterJob,
        config: &Config,
        on_page: &mut (dyn FnMut(PageProgress) + Send),
    ) -> impl Future<Output = Result<ConversionSummary, ConvertError>> + Send;

    /// The state of the two API keys of `config`, from the one free call each provider allows. It
    /// costs nothing, and it never fails: a check that cannot tell is a state.
    fn check_keys(&self, config: &Config) -> impl Future<Output = KeyStates> + Send;

    /// The saved model and thinking of one kind of work.
    ///
    /// # Errors
    /// [`SettingsError`] when the saved choice cannot be read or is not valid.
    fn model_choice(&self, work: WorkKind) -> Result<ModelSettings, SettingsError>;
}

/// The paid services. It keeps the clients that ask the providers about a key, so each is made
/// once for the whole run of the server.
#[derive(Default)]
pub struct PaidServices {
    key_clients: KeyClients,
}

impl Services for PaidServices {
    type Embedder = GeminiEmbedder;
    type Llm = AgentCli;

    fn embedder(&self, config: &Config) -> Result<GeminiEmbedder, EmbedError> {
        GeminiEmbedder::from_config(config)
    }

    fn llm(&self, work: WorkKind) -> AgentCli {
        AgentCli::for_work(work)
    }

    async fn convert(
        &self,
        job: &ChapterJob,
        config: &Config,
        on_page: &mut (dyn FnMut(PageProgress) + Send),
    ) -> Result<ConversionSummary, ConvertError> {
        // The key goes to the converter as a value, because the settings never enter the process
        // environment.
        let jev_api_key = config.jev_api_key.as_ref().map(ApiKey::expose);
        ocr::convert_chapter_with_jev_key(job, jev_api_key, on_page).await
    }

    fn check_keys(&self, config: &Config) -> impl Future<Output = KeyStates> + Send {
        self.key_clients.check(config)
    }

    fn model_choice(&self, work: WorkKind) -> Result<ModelSettings, SettingsError> {
        ModelSettings::load_for(work)
    }
}
