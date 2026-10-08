//! Runs one traced search over throwaway stores filled by hand, so that what each step must
//! produce is known beforehand, with an embedder that makes up its vectors, so nothing is billed.

use std::path::Path;

use graph::{ConceptNode, RelationKind};
use rag_core::{ConceptId, DocumentLabels, ItemHit};
use rag_ingestion::testing::ThrowawayStores;
use rag_retrieval::{RESULTS_PER_QUERY, Reason, Retriever, SearchTrace};

use crate::support::{Fixture, Placed, QUESTION, SCORE_ERROR, found_among, tagged, texts_of};

fn assert_send<T: Send>(value: T) -> T {
    value
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-retrieval --test integration -- --ignored traced_search::"]
async fn a_traced_search_gives_the_results_of_a_search_and_what_each_step_produced() {
    let throwaway = ThrowawayStores::new("search-traced");
    let stores = throwaway.connect().await;
    let mut fixture = Fixture::new();
    let [document_a, document_b, document_c, document_d] =
        ["A", "B", "C", "D"].map(|name| fixture.document(&format!("Document {name}")));
    // Document A has one more chunk than there are seeds, and its first chunk cites a figure.
    let a1 = fixture.add(Placed::chunk(document_a, "a1", 0.90).citing(&["Figure 13-4"]));
    for index in 1..=RESULTS_PER_QUERY {
        let name = format!("a{}", index + 1);
        fixture.add(Placed::chunk(document_a, &name, 0.90 - 0.01 * index as f32));
    }
    fixture.add(Placed::figure(
        document_a,
        "figure of A",
        0.05,
        "Figure 13-4",
        Path::new("figures/figure-13-4.png"),
    ));
    let b1 = fixture.add(Placed::chunk(document_b, "b1", 0.30));
    let c1 = fixture.add(Placed::chunk(document_c, "c1", 0.20));
    let d1 = fixture.add(Placed::chunk(document_d, "d1", 0.10));
    fixture.label(document_a, tagged(&["options"]));
    fixture.label(document_b, tagged(&["futures"]));
    let black_scholes = fixture.concept("Black–Scholes model", 0.10);
    let ito = fixture.concept("Itô's lemma", 0.05);
    let parity = fixture.concept("put–call parity", 0.90);
    fixture.concept_point("a decoy that no item mentions", 0.80);
    fixture.concept_point("another decoy that no item mentions", 0.70);
    fixture.mention(a1, black_scholes);
    fixture.mention(b1, black_scholes);
    fixture.mention(c1, ito);
    fixture.mention(d1, parity);
    fixture.relate(a1, black_scholes, RelationKind::DerivedFrom, ito);
    fixture.store_in(&stores).await;
    let retriever = Retriever {
        embedder: Fixture::embedder(),
        items: stores.items,
        concepts: stores.concepts,
        graph: stores.graph,
    };
    let texts = |hits: &[ItemHit]| -> Vec<String> {
        hits.iter().map(|hit| hit.payload.text.clone()).collect()
    };
    let ids = |concepts: &[ConceptNode]| -> Vec<ConceptId> {
        concepts.iter().map(|concept| concept.id).collect()
    };

    let no_labels = DocumentLabels::default();
    let traced = assert_send(retriever.search_traced(QUESTION, None, &no_labels))
        .await
        .unwrap();

    assert_eq!(
        traced.results,
        found_among(&retriever, None, &no_labels).await,
        "a search is a traced search without its trace"
    );
    assert_eq!(
        texts_of(&traced.results),
        ["a1", "a2", "a3", "b1", "c1", "d1", "figure of A"]
    );
    let trace = &traced.trace;
    assert_eq!(trace.documents_searched, None);
    assert_eq!(
        texts(&trace.seeds),
        ["a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8"]
    );
    let question_concepts: Vec<&str> = trace
        .question_concepts
        .iter()
        .map(|concept| concept.name.as_str())
        .collect();
    assert_eq!(
        question_concepts,
        [
            "put–call parity",
            "a decoy that no item mentions",
            "another decoy that no item mentions"
        ]
    );
    assert_eq!(trace.question_concepts[0].id, parity);
    assert!((trace.question_concepts[0].score - 0.90).abs() < SCORE_ERROR);
    assert_eq!(ids(&trace.seed_concepts), [black_scholes]);
    assert_eq!(ids(&trace.related_concepts), [ito]);
    assert_eq!(trace.candidates, 11, "eight seeds, and b1, c1 and d1");
    assert_eq!(trace.ranked, 11);
    assert_eq!(texts(&trace.capped), ["a4", "a5", "a6", "a7", "a8"]);
    assert_eq!(trace.kept, 6);
    assert!(matches!(
        traced.results.hits[trace.kept].reason,
        Reason::Cited { by: 1, .. }
    ));

    // Only document A is tagged `options`. The graph still adds b1, c1 and d1, and they are left
    // out where the candidates are ranked.
    let options = tagged(&["options"]);
    let traced = retriever
        .search_traced(QUESTION, None, &options)
        .await
        .unwrap();
    assert_eq!(
        traced.results,
        found_among(&retriever, None, &options).await
    );
    assert_eq!(texts_of(&traced.results), ["a1", "a2", "a3", "figure of A"]);
    assert_eq!(traced.trace.documents_searched, Some(1));
    assert_eq!(traced.trace.candidates, 11);
    assert_eq!(traced.trace.ranked, 8);
    assert_eq!(texts(&traced.trace.capped), ["a4", "a5", "a6", "a7", "a8"]);
    assert_eq!(traced.trace.kept, 3);

    let traced = retriever
        .search_traced(QUESTION, None, &tagged(&["rates"]))
        .await
        .unwrap();
    assert!(traced.results.hits.is_empty());
    assert_eq!(
        traced.trace,
        SearchTrace {
            documents_searched: Some(0),
            ..SearchTrace::default()
        }
    );
}
