//! The outside services that one tool call needs, behind one trait: `main` hands the paid ones to
//! the server and a test hands stand-ins, so no test can reach a paid service.

use std::future::Future;

use ocr::{ChapterJob, ConversionSummary, ConvertError};
use rag_core::{ApiKey, ClaudeCli, Config, EmbedError, Embedder, GeminiEmbedder, Llm};

/// Makes the outside services of one tool call. The graph is not part of it: a call always opens
/// the FalkorDB graph that its settings name, and a test points those settings at a throwaway one.
pub trait Services: Send + Sync + 'static {
    type Embedder: Embedder + Send + Sync + 'static;
    type Llm: Llm + Send + Sync + 'static;

    /// # Errors
    /// [`EmbedError::MissingApiKey`] when the settings hold no key for the embedder.
    fn embedder(&self, config: &Config) -> Result<Self::Embedder, EmbedError>;

    /// The model with this name: `rag_retrieval::ANSWER_MODEL` or
    /// `rag_ingestion::EXTRACTION_MODEL`.
    fn llm(&self, model: &str) -> Self::Llm;

    /// Converts the pages of the chapter that are not converted yet. This is the part of an ingest
    /// that pays for a model call for every page.
    ///
    /// # Errors
    /// The errors of `ocr::convert_chapter_with_jev_key`.
    fn convert(
        &self,
        job: &ChapterJob,
        config: &Config,
    ) -> impl Future<Output = Result<ConversionSummary, ConvertError>> + Send;
}

pub struct PaidServices;

impl Services for PaidServices {
    type Embedder = GeminiEmbedder;
    type Llm = ClaudeCli;

    fn embedder(&self, config: &Config) -> Result<GeminiEmbedder, EmbedError> {
        GeminiEmbedder::from_config(config)
    }

    fn llm(&self, model: &str) -> ClaudeCli {
        ClaudeCli::new(model)
    }

    async fn convert(
        &self,
        job: &ChapterJob,
        config: &Config,
    ) -> Result<ConversionSummary, ConvertError> {
        // The key goes to the converter as a value, because the settings never enter the process
        // environment.
        let jev_api_key = config.jev_api_key.as_ref().map(ApiKey::expose);
        ocr::convert_chapter_with_jev_key(job, jev_api_key, |_| {}).await
    }
}
