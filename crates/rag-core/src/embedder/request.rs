//! The JSON that goes to the embeddings API and comes back, and the two text formats that make
//! a stored item and a question land near each other.

use std::path::Path;

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::{DocumentInput, EMBEDDING_DIMENSIONS, EMBEDDING_MODEL, EmbedError, Embedding};

/// The model has no task setting, so what a text is for is written into the text.
pub(super) fn document_text(input: &DocumentInput) -> String {
    let title = if input.title.trim().is_empty() {
        "none"
    } else {
        input.title.as_str()
    };
    format!("title: {title} | text: {}", input.text)
}

pub(super) fn query_text(query: &str) -> String {
    format!("task: search result | query: {query}")
}

#[derive(Serialize)]
pub(super) struct EmbedBody {
    content: Content,
    output_dimensionality: usize,
}

impl EmbedBody {
    pub(super) fn text(text: String) -> Self {
        Self::new(vec![Part::Text { text }])
    }

    /// The text part comes first and the picture second. The API merges them into one vector.
    pub(super) fn text_with_image(text: String, image: &Path) -> Result<Self, EmbedError> {
        let mime_type = mime_type(image).ok_or_else(|| EmbedError::UnsupportedImage {
            path: image.to_path_buf(),
        })?;
        let bytes = std::fs::read(image).map_err(|source| EmbedError::Image {
            path: image.to_path_buf(),
            source,
        })?;
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        Ok(Self::new(vec![
            Part::Text { text },
            Part::Image {
                inline_data: InlineData { mime_type, data },
            },
        ]))
    }

    fn new(parts: Vec<Part>) -> Self {
        Self {
            content: Content { parts },
            output_dimensionality: EMBEDDING_DIMENSIONS,
        }
    }
}

#[derive(Serialize)]
pub(super) struct BatchBody {
    requests: Vec<BatchRequest>,
}

impl BatchBody {
    pub(super) fn of_texts(texts: Vec<String>) -> Self {
        let requests = texts
            .into_iter()
            .map(|text| BatchRequest {
                model: format!("models/{EMBEDDING_MODEL}"),
                content: Content {
                    parts: vec![Part::Text { text }],
                },
                output_dimensionality: EMBEDDING_DIMENSIONS,
            })
            .collect();
        Self { requests }
    }
}

#[derive(Serialize)]
struct BatchRequest {
    model: String,
    content: Content,
    output_dimensionality: usize,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    Image { inline_data: InlineData },
}

#[derive(Serialize)]
struct InlineData {
    mime_type: &'static str,
    data: String,
}

fn mime_type(image: &Path) -> Option<&'static str> {
    let extension = image.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        _ => None,
    }
}

#[derive(Deserialize)]
pub(super) struct EmbedReply {
    embedding: Values,
}

impl EmbedReply {
    pub(super) fn into_vector(self) -> Result<Embedding, EmbedError> {
        self.embedding.checked()
    }
}

#[derive(Deserialize)]
pub(super) struct BatchReply {
    embeddings: Vec<Values>,
}

impl BatchReply {
    /// A reply with another number of vectors than there were texts is an error, never a
    /// shorter list, because the vectors would then belong to the wrong inputs.
    pub(super) fn into_vectors(self, expected: usize) -> Result<Vec<Embedding>, EmbedError> {
        if self.embeddings.len() != expected {
            return Err(EmbedError::WrongVectorCount {
                expected,
                got: self.embeddings.len(),
            });
        }
        self.embeddings.into_iter().map(Values::checked).collect()
    }
}

#[derive(Deserialize)]
struct Values {
    values: Vec<f32>,
}

impl Values {
    fn checked(self) -> Result<Embedding, EmbedError> {
        if self.values.len() != EMBEDDING_DIMENSIONS {
            return Err(EmbedError::WrongVectorLength {
                expected: EMBEDDING_DIMENSIONS,
                got: self.values.len(),
            });
        }
        Ok(self.values)
    }
}
