//! An embedder that makes up its vectors, and keeps every input it is given, so that a test sees
//! what was embedded. A test can also place chosen inputs at a chosen distance from each other.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, MutexGuard};

use rag_core::{DocumentInput, EMBEDDING_DIMENSIONS, EmbedError, Embedder, Embedding};

/// Keeps every input it is given, and answers each with a vector that depends on the input, so
/// that different items get different points. An input or a question whose text was placed gets
/// the vector it was placed at.
#[derive(Default)]
pub struct StandInEmbedder {
    received: Mutex<Vec<DocumentInput>>,
    placed: HashMap<String, Embedding>,
}

impl StandInEmbedder {
    pub fn received(&self) -> Vec<DocumentInput> {
        self.locked_inputs().clone()
    }

    fn locked_inputs(&self) -> MutexGuard<'_, Vec<DocumentInput>> {
        self.received
            .lock()
            .expect("the lock of the received inputs should not be poisoned")
    }

    /// Every input and every question whose text is exactly `text` gets `vector`.
    pub fn placing(mut self, text: &str, vector: Embedding) -> StandInEmbedder {
        self.placed.insert(text.to_owned(), vector);
        self
    }
}

impl Embedder for StandInEmbedder {
    async fn embed_document(&self, inputs: &[DocumentInput]) -> Result<Vec<Embedding>, EmbedError> {
        self.locked_inputs().extend(inputs.iter().cloned());
        Ok(inputs
            .iter()
            .map(|input| match self.placed.get(&input.text) {
                Some(vector) => vector.clone(),
                None => unit_vector(&format!("{}|{}|{:?}", input.title, input.text, input.image)),
            })
            .collect())
    }

    async fn embed_query(&self, query: &str) -> Result<Embedding, EmbedError> {
        Ok(match self.placed.get(query) {
            Some(vector) => vector.clone(),
            None => unit_vector(query),
        })
    }
}

/// The vector that every other vector is placed against: all of it on the first axis.
pub fn first_axis() -> Embedding {
    vector_at(1.0, 1)
}

/// A unit vector whose cosine to [`first_axis`] is `cosine`: that much on the first axis and the
/// rest on `other_axis`. Two such vectors on different other axes have the cosine `c1 * c2` to
/// each other, so give each placed name its own other axis.
pub fn vector_at(cosine: f32, other_axis: usize) -> Embedding {
    assert!(
        (1..EMBEDDING_DIMENSIONS).contains(&other_axis),
        "the other axis must not be the first one"
    );
    let mut vector = vec![0.0; EMBEDDING_DIMENSIONS];
    vector[0] = cosine;
    vector[other_axis] = (1.0 - cosine * cosine).sqrt();
    vector
}

fn unit_vector(seed_text: &str) -> Embedding {
    let mut hasher = DefaultHasher::new();
    seed_text.hash(&mut hasher);
    let mut state = hasher.finish();
    let numbers: Vec<f32> = (0..EMBEDDING_DIMENSIONS)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 40) as f32 / (1u64 << 24) as f32 - 0.5
        })
        .collect();
    let norm = numbers
        .iter()
        .map(|number| number * number)
        .sum::<f32>()
        .sqrt();
    numbers.into_iter().map(|number| number / norm).collect()
}
