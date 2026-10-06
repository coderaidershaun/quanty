//! The embedder that calls the Gemini embeddings API: batches texts, sends each picture on its
//! own, and tries again when the API is busy.

use std::fmt;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::request::{BatchBody, BatchReply, EmbedBody, EmbedReply, document_text, query_text};
use super::{DocumentInput, EMBEDDING_MODEL, EmbedError, Embedder, Embedding};
use crate::{ApiKey, Config};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_FIRST_RETRY_DELAY: Duration = Duration::from_secs(1);
/// The most texts the API takes in one batch. It refuses 101 with HTTP 400.
const MAX_BATCH_SIZE: usize = 100;
/// This count includes the first try.
const MAX_TRIES: u32 = 5;

/// Embeds with Gemini over HTTP. Its `Debug` output shows no key.
pub struct GeminiEmbedder {
    client: reqwest::Client,
    api_key: ApiKey,
    base_url: String,
    first_retry_delay: Duration,
}

impl GeminiEmbedder {
    /// # Errors
    /// [`EmbedError::Transport`] when the HTTP client cannot be built.
    pub fn new(api_key: &ApiKey) -> Result<Self, EmbedError> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self {
            client,
            api_key: api_key.clone(),
            base_url: DEFAULT_BASE_URL.to_owned(),
            first_retry_delay: DEFAULT_FIRST_RETRY_DELAY,
        })
    }

    /// # Errors
    /// [`EmbedError::MissingApiKey`] when the config has no key.
    pub fn from_config(config: &Config) -> Result<Self, EmbedError> {
        let key = config
            .gemini_api_key
            .as_ref()
            .ok_or(EmbedError::MissingApiKey)?;
        Self::new(key)
    }

    /// Points the embedder at another server, such as a stand-in in a test. `base_url` is
    /// everything before `/models/`. The wait before the second try is `first_retry_delay`, and
    /// each later wait is twice the one before.
    pub fn with_endpoint(mut self, base_url: &str, first_retry_delay: Duration) -> Self {
        self.base_url = base_url.trim_end_matches('/').to_owned();
        self.first_retry_delay = first_retry_delay;
        self
    }

    async fn embed_texts(&self, texts: Vec<String>) -> Result<Vec<Embedding>, EmbedError> {
        let expected = texts.len();
        let reply: BatchReply = self
            .post("batchEmbedContents", &BatchBody::of_texts(texts))
            .await?;
        reply.into_vectors(expected)
    }

    async fn embed_one(&self, body: &EmbedBody) -> Result<Embedding, EmbedError> {
        let reply: EmbedReply = self.post("embedContent", body).await?;
        reply.into_vector()
    }

    /// Sends one request, and sends it again after a wait when the failure may pass: a lost
    /// connection, a timeout, HTTP 429 or HTTP 5xx.
    async fn post<Body: Serialize, Reply: DeserializeOwned>(
        &self,
        method: &str,
        body: &Body,
    ) -> Result<Reply, EmbedError> {
        let url = format!("{}/models/{EMBEDDING_MODEL}:{method}", self.base_url);
        let mut wait = self.first_retry_delay;
        let mut attempt = 1;
        loop {
            let error = match self.post_once(&url, body).await {
                Ok(reply) => return Ok(reply),
                Err(error) => error,
            };
            let reason = match retry_reason(&error) {
                Some(reason) if attempt < MAX_TRIES => reason,
                _ => return Err(error),
            };
            tracing::warn!(
                attempt,
                method,
                reason,
                "embeddings request failed, trying again"
            );
            tokio::time::sleep(wait).await;
            wait *= 2;
            attempt += 1;
        }
    }

    async fn post_once<Body: Serialize, Reply: DeserializeOwned>(
        &self,
        url: &str,
        body: &Body,
    ) -> Result<Reply, EmbedError> {
        let response = self
            .client
            .post(url)
            .header("x-goog-api-key", self.api_key.expose())
            .json(body)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(EmbedError::Rejected {
                status: status.as_u16(),
                body: text,
            });
        }
        serde_json::from_str(&text)
            .map_err(|source| EmbedError::UnreadableReply { body: text, source })
    }
}

/// A short description of a failure that is worth another try, or `None` for one that is not.
/// It leaves out the reply body, so it is safe to log.
fn retry_reason(error: &EmbedError) -> Option<String> {
    match error {
        EmbedError::Transport(source) if !source.is_builder() => Some(source.to_string()),
        EmbedError::Rejected { status, .. } if *status == 429 || *status >= 500 => {
            Some(format!("http status {status}"))
        }
        _ => None,
    }
}

impl fmt::Debug for GeminiEmbedder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeminiEmbedder")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl Embedder for GeminiEmbedder {
    /// Every picture is read before the first request, so a picture that is missing fails the
    /// call before anything is billed. The calls are made one after the other, because parallel
    /// calls only meet the rate limit sooner.
    async fn embed_document(&self, inputs: &[DocumentInput]) -> Result<Vec<Embedding>, EmbedError> {
        let mut text_only: Vec<(usize, &DocumentInput)> = Vec::new();
        let mut with_picture: Vec<(usize, EmbedBody)> = Vec::new();
        for (position, input) in inputs.iter().enumerate() {
            match &input.image {
                Some(image) => {
                    let text = document_text(input);
                    with_picture.push((position, EmbedBody::text_with_image(text, image)?));
                }
                None => text_only.push((position, input)),
            }
        }

        let mut placed: Vec<(usize, Embedding)> = Vec::with_capacity(inputs.len());
        for group in text_only.chunks(MAX_BATCH_SIZE) {
            let texts = group
                .iter()
                .map(|(_, input)| document_text(input))
                .collect();
            let vectors = self.embed_texts(texts).await?;
            placed.extend(group.iter().map(|(position, _)| *position).zip(vectors));
        }
        for (position, body) in with_picture {
            placed.push((position, self.embed_one(&body).await?));
        }

        placed.sort_by_key(|(position, _)| *position);
        Ok(placed.into_iter().map(|(_, vector)| vector).collect())
    }

    async fn embed_query(&self, query: &str) -> Result<Embedding, EmbedError> {
        self.embed_one(&EmbedBody::text(query_text(query))).await
    }
}
