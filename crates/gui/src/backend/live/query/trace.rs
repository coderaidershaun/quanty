//! Turns what each step of a search produced into the six rows of the retrieval path, and lists
//! the concepts the search started from, which the concept graph starts from too.

use std::collections::HashMap;

use rag_core::{ConceptId, ItemHit};
use rag_retrieval::{Reason, SearchHit, SearchTrace};

use crate::contract::RetrievalTrace;

/// A concept the search started from.
pub(super) struct SeedConcept {
    pub(super) id: ConceptId,
    pub(super) name: String,
    /// The search gives it only for a concept that one of the nearest items mentions.
    pub(super) definition: Option<String>,
}

/// The concepts the search started from: those nearest to the question, nearest first, and then
/// the concepts the nearest items mention that are not among them.
pub(super) fn seeds(trace: &SearchTrace) -> Vec<SeedConcept> {
    let definitions: HashMap<ConceptId, &str> = trace
        .seed_concepts
        .iter()
        .map(|node| (node.id, node.definition.as_str()))
        .collect();
    let mut seeds: Vec<SeedConcept> = trace
        .question_concepts
        .iter()
        .map(|hit| SeedConcept {
            id: hit.id,
            name: hit.name.clone(),
            definition: definitions
                .get(&hit.id)
                .map(|definition| (*definition).to_owned()),
        })
        .collect();
    for node in &trace.seed_concepts {
        if seeds.iter().all(|seed| seed.id != node.id) {
            seeds.push(SeedConcept {
                id: node.id,
                name: node.name.clone(),
                definition: Some(node.definition.clone()),
            });
        }
    }
    seeds
}

/// A search that found no item reached no later step, and that is not the same as a step that
/// found nothing: its rows are `None`, not zero.
pub(super) fn view(trace: &SearchTrace, hits: &[SearchHit]) -> RetrievalTrace {
    if trace.seeds.is_empty() {
        return RetrievalTrace {
            documents_searched: trace.documents_searched,
            ..RetrievalTrace::default()
        };
    }
    RetrievalTrace {
        documents_searched: trace.documents_searched,
        nearest: trace.seeds.len(),
        seed_concepts: Some(seeds(trace).into_iter().map(|seed| seed.name).collect()),
        related_concepts: Some(
            trace
                .related_concepts
                .iter()
                .map(|node| node.name.clone())
                .collect(),
        ),
        candidates: Some(trace.candidates),
        ranked: Some(trace.ranked),
        kept: Some(trace.kept),
        passed_over: passed_over(&trace.capped),
        cited: Some(cited_labels(hits.get(trace.kept..).unwrap_or_default())),
    }
}

fn passed_over(capped: &[ItemHit]) -> Vec<(String, usize)> {
    let mut documents: Vec<(String, usize)> = Vec::new();
    for hit in capped {
        let title = &hit.payload.doc_title;
        match documents.iter_mut().find(|(known, _)| known == title) {
            Some((_, count)) => *count += 1,
            None => documents.push((title.clone(), 1)),
        }
    }
    documents
}

fn cited_labels(pulled_in: &[SearchHit]) -> Vec<String> {
    pulled_in
        .iter()
        .filter_map(|hit| match &hit.reason {
            Reason::Cited { label, .. } => Some(label.clone()),
            _ => None,
        })
        .collect()
}
