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

/// The documents that lost items to the cap, in the order of their first lost item, each with
/// how many it lost.
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

#[cfg(test)]
mod tests {
    use graph::ConceptNode;
    use rag_core::{ConceptHit, DocId, DocumentLabels, ItemHit, ItemId, ItemKind, ItemPayload};
    use rag_retrieval::Reason;

    use super::*;

    fn concept(name: &str) -> ConceptNode {
        ConceptNode {
            id: ConceptId::random(),
            name: name.to_owned(),
            normalised_name: name.to_lowercase(),
            definition: format!("what {name} is"),
        }
    }

    fn nearest_to_the_question(node: &ConceptNode) -> ConceptHit {
        ConceptHit {
            id: node.id,
            score: 0.9,
            name: node.name.clone(),
            aliases: Vec::new(),
        }
    }

    fn item_of(doc_title: &str) -> ItemHit {
        let doc_id = DocId::from_source_sha256(doc_title);
        ItemHit {
            id: ItemId::new(doc_id, ItemKind::Chunk, 0),
            score: 0.5,
            payload: ItemPayload {
                doc_id,
                doc_title: doc_title.to_owned(),
                page: 1,
                printed_page: None,
                kind: ItemKind::Chunk,
                text: String::new(),
                image_path: None,
                label: None,
                cites: Vec::new(),
                document_labels: DocumentLabels::default(),
            },
        }
    }

    fn hit(reason: Reason) -> SearchHit {
        SearchHit {
            item: item_of("Any book"),
            reason,
        }
    }

    fn cited(label: &str) -> Reason {
        Reason::Cited {
            by: 1,
            label: label.to_owned(),
        }
    }

    #[test]
    fn a_search_with_no_seed_reports_no_later_step_and_a_full_one_reports_all_six() {
        let stopped = SearchTrace {
            documents_searched: Some(0),
            ..SearchTrace::default()
        };

        assert_eq!(
            view(&stopped, &[]),
            RetrievalTrace {
                documents_searched: Some(0),
                nearest: 0,
                seed_concepts: None,
                related_concepts: None,
                candidates: None,
                ranked: None,
                kept: None,
                passed_over: Vec::new(),
                cited: None,
            }
        );

        let [black_scholes, put_call, ito, volatility, rho] = [
            "Black-Scholes",
            "Put-call parity",
            "Ito",
            "Volatility",
            "Rho",
        ]
        .map(concept);
        let full = SearchTrace {
            documents_searched: None,
            seeds: vec![item_of("A"), item_of("A"), item_of("B")],
            question_concepts: vec![
                nearest_to_the_question(&black_scholes),
                nearest_to_the_question(&put_call),
            ],
            seed_concepts: vec![ito, black_scholes],
            related_concepts: vec![volatility, rho],
            candidates: 10,
            ranked: 9,
            capped: vec![item_of("A"), item_of("B"), item_of("A")],
            kept: 2,
        };
        let hits = [
            hit(Reason::Nearest),
            hit(cited("(9.9)")),
            hit(cited("(2.4)")),
            hit(cited("Table 1-1")),
        ];

        assert_eq!(
            view(&full, &hits),
            RetrievalTrace {
                documents_searched: None,
                nearest: 3,
                seed_concepts: Some(vec![
                    "Black-Scholes".to_owned(),
                    "Put-call parity".to_owned(),
                    "Ito".to_owned(),
                ]),
                related_concepts: Some(vec!["Volatility".to_owned(), "Rho".to_owned()]),
                candidates: Some(10),
                ranked: Some(9),
                kept: Some(2),
                passed_over: vec![("A".to_owned(), 2), ("B".to_owned(), 1)],
                cited: Some(vec!["(2.4)".to_owned(), "Table 1-1".to_owned()]),
            }
        );
    }
}
