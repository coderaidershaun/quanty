//! An embedder that reads words: a question that repeats the words of an item lands near that item.

use std::sync::{Arc, Mutex};

use rag_core::{DocumentInput, EMBEDDING_DIMENSIONS, EmbedError, Embedder, Embedding};

// SMELL: this is a stand-in that knows nothing of meaning. Every word counts the same and many
// words share one place in the vector, so only a question that is the whole stored text of an
// item is sure to find it. It shows that search is wired up, not that it finds good answers.
#[derive(Clone, Default)]
pub struct WordEmbedder {
    questions: Arc<Mutex<Vec<String>>>,
}

impl WordEmbedder {
    pub fn questions(&self) -> Vec<String> {
        self.questions.lock().unwrap().clone()
    }
}

impl Embedder for WordEmbedder {
    async fn embed_document(&self, inputs: &[DocumentInput]) -> Result<Vec<Embedding>, EmbedError> {
        Ok(inputs
            .iter()
            .map(|input| word_vector(&format!("{} {}", input.title, input.text)))
            .collect())
    }

    async fn embed_query(&self, query: &str) -> Result<Embedding, EmbedError> {
        self.questions.lock().unwrap().push(query.to_owned());
        Ok(word_vector(query))
    }
}

fn word_vector(text: &str) -> Embedding {
    let mut vector = vec![0.0_f32; EMBEDDING_DIMENSIONS];
    let lowercase = text.to_lowercase();
    let words = lowercase
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty());
    for word in words {
        vector[(fnv1a(word) % EMBEDDING_DIMENSIONS as u64) as usize] += 1.0;
    }
    let norm = vector.iter().map(|count| count * count).sum::<f32>().sqrt();
    if norm == 0.0 {
        vector[0] = 1.0;
        return vector;
    }
    vector.into_iter().map(|count| count / norm).collect()
}

/// Written out here and not taken from the standard library, because the standard hasher may
/// give other numbers in another Rust release, and these tests need the same vectors on every
/// run.
fn fnv1a(word: &str) -> u64 {
    word.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}
