//! Asks questions of throwaway stores: one that holds the three committed chapters, and others
//! that are filled by hand with items at known distances from the question, with an embedder that
//! makes up its vectors, so nothing is billed.

use std::path::Path;

use graph::{ConceptNode, FalkorGraph, RelationKind};
use rag_core::{ConceptId, DocumentLabels, ItemHit, ItemId, ItemKind};
use rag_ingestion::testing::{StandInEmbedder, ThrowawayStores};
use rag_retrieval::{
    MAX_RESULTS_PER_DOCUMENT, RESULTS_PER_QUERY, Reason, Retriever, SearchResults, SearchTrace,
};

use crate::support::{
    self, Fixture, Placed, QUESTION, SAMPLE_CHAPTER_TITLE, WordEmbedder, find_item, store_samples,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-retrieval --test integration -- --ignored search::"]
async fn a_question_finds_its_item_and_prints_title_page_kind_text_and_picture() {
    let throwaway = ThrowawayStores::new("search");
    let stores = throwaway.connect().await;
    let embedder = WordEmbedder::default();
    let items = store_samples(&embedder, &stores.items, &stores.concepts).await;
    let target = find_item(&items, SAMPLE_CHAPTER_TITLE, ItemKind::Figure, 5);
    let question = target.payload.text.clone();
    let retriever = Retriever {
        embedder: embedder.clone(),
        items: stores.items,
        concepts: stores.concepts,
        graph: stores.graph,
    };

    let results = retriever
        .search(&question, None, &DocumentLabels::default())
        .await
        .unwrap();

    assert_eq!(embedder.questions(), vec![question.clone()]);
    let ranked: Vec<_> = results
        .hits
        .iter()
        .filter(|hit| !matches!(hit.reason, Reason::Cited { .. }))
        .collect();
    assert!(!ranked.is_empty() && ranked.len() <= RESULTS_PER_QUERY);
    assert!(
        ranked
            .windows(2)
            .all(|pair| pair[0].item.score >= pair[1].item.score),
        "the scores should not rise"
    );
    let first = &results.hits[0].item;
    assert_eq!(first.id, target.id);
    assert_eq!(first.payload, target.payload);
    assert_eq!(first.payload.kind, ItemKind::Figure);
    assert_eq!(first.payload.page, 5);
    assert_eq!(first.payload.printed_page.as_deref(), Some("233"));
    let picture = std::fs::canonicalize(support::sample_chapter())
        .unwrap()
        .join("page-num-5/01-figure.png");
    assert_eq!(first.payload.image_path.as_deref(), Some(picture.as_path()));
    assert!(picture.is_file());

    let printed = results.to_string();
    let header: Vec<&str> = printed
        .lines()
        .take_while(|line| *line != "text:")
        .collect();
    assert!(header.contains(&format!("document: {SAMPLE_CHAPTER_TITLE}").as_str()));
    assert!(header.contains(&"page: 233"));
    assert!(header.contains(&"kind: figure"));
    assert!(header.contains(&format!("picture: {}", picture.display()).as_str()));
    assert!(printed.contains(&first.payload.text));
}

/// How far the score that Qdrant gives an item may be from the cosine the item was placed at.
const SCORE_ERROR: f32 = 0.001;

/// Labels that name nothing but these tags.
fn tagged(tags: &[&str]) -> DocumentLabels {
    DocumentLabels {
        tags: tags.iter().map(|tag| tag.parse().unwrap()).collect(),
        ..DocumentLabels::default()
    }
}

async fn found_among(
    retriever: &Retriever<StandInEmbedder, FalkorGraph>,
    kind: Option<ItemKind>,
    wanted: &DocumentLabels,
) -> SearchResults {
    retriever.search(QUESTION, kind, wanted).await.unwrap()
}

fn texts_of(results: &SearchResults) -> Vec<&str> {
    results
        .hits
        .iter()
        .map(|hit| hit.item.payload.text.as_str())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-retrieval --test integration -- --ignored search::"]
async fn an_item_of_another_document_comes_back_through_a_shared_concept_and_the_cap_holds() {
    let throwaway = ThrowawayStores::new("search-graph");
    let stores = throwaway.connect().await;
    let mut fixture = Fixture::new();
    let [document_a, document_b, document_c, document_d, document_e] =
        ["A", "B", "C", "D", "E"].map(|name| fixture.document(&format!("Document {name}")));
    // Document A has one more chunk than there are seeds, so its last chunk is not a seed.
    let a: Vec<ItemId> = (0..=RESULTS_PER_QUERY)
        .map(|index| {
            let name = format!("a{}", index + 1);
            fixture.add(Placed::chunk(document_a, &name, 0.90 - 0.01 * index as f32))
        })
        .collect();
    let b1 = fixture.add(Placed::chunk(document_b, "b1", 0.30));
    let c1 = fixture.add(Placed::chunk(document_c, "c1", 0.20));
    let d1 = fixture.add(Placed::chunk(document_d, "d1", 0.10));
    fixture.add(Placed::chunk(document_e, "e1", 0.50));
    fixture.label(
        document_a,
        DocumentLabels {
            book: Some("Pricing Options".to_owned()),
            author: Some("Sheldon Natenberg".to_owned()),
            ..tagged(&["options"])
        },
    );
    fixture.label(document_b, tagged(&["futures"]));
    fixture.label(document_c, tagged(&["options"]));
    fixture.label(document_d, tagged(&["futures"]));
    fixture.label(document_e, tagged(&["rates"]));
    // The three concepts nearest to the question are the put-call parity and the two decoys, so
    // each of the other two concepts can only be reached through the graph.
    let black_scholes = fixture.concept("Black–Scholes model", 0.10);
    let ito = fixture.concept("Itô's lemma", 0.05);
    let parity = fixture.concept("put–call parity", 0.90);
    fixture.concept_point("a decoy that no item mentions", 0.80);
    fixture.concept_point("another decoy that no item mentions", 0.70);
    fixture.mention(a[0], black_scholes);
    fixture.mention(b1, black_scholes);
    fixture.mention(c1, ito);
    fixture.mention(d1, parity);
    fixture.relate(a[0], black_scholes, RelationKind::DerivedFrom, ito);
    fixture.store_in(&stores).await;
    let retriever = Retriever {
        embedder: Fixture::embedder(),
        items: stores.items,
        concepts: stores.concepts,
        graph: stores.graph,
    };

    let results = found_among(&retriever, None, &DocumentLabels::default()).await;

    let found: Vec<&str> = results
        .hits
        .iter()
        .map(|hit| hit.item.payload.text.as_str())
        .collect();
    assert_eq!(
        found,
        ["a1", "a2", "a3", "b1", "c1", "d1"],
        "the cap keeps a1 to a3; b1, c1 and d1 each come through the graph; e1 is nearer than b1 but no concept leads to it"
    );
    let expected_scores = [0.90, 0.89, 0.88, 0.30, 0.20, 0.10];
    for (hit, expected) in results.hits.iter().zip(expected_scores) {
        assert!(
            (hit.item.score - expected).abs() < SCORE_ERROR,
            "{} scored {}, not {expected}",
            hit.item.payload.text,
            hit.item.score
        );
    }
    let reasons: Vec<&Reason> = results.hits.iter().map(|hit| &hit.reason).collect();
    assert_eq!(
        reasons,
        [
            &Reason::Nearest,
            &Reason::Nearest,
            &Reason::Nearest,
            &Reason::Concept("Black–Scholes model".to_owned()),
            &Reason::Concept("Itô's lemma".to_owned()),
            &Reason::Concept("put–call parity".to_owned()),
        ]
    );
    let of_document_a = results
        .hits
        .iter()
        .filter(|hit| hit.item.payload.doc_title == "Document A")
        .count();
    assert_eq!(
        of_document_a, MAX_RESULTS_PER_DOCUMENT,
        "eight chunks of document A are nearer than b1"
    );
    let printed = results.to_string();
    let block_of_b1 = printed.split("\n\n").nth(3).unwrap();
    assert!(block_of_b1.starts_with("result 4\n"), "{block_of_b1}");
    assert!(
        block_of_b1
            .lines()
            .any(|line| line == "reached via concept Black–Scholes model"),
        "{block_of_b1}"
    );

    // Document E is behind a1 to a9 and no concept leads to it, so only a seed query that looks at
    // the labels finds it.
    let results = found_among(&retriever, None, &tagged(&["rates"])).await;
    assert_eq!(texts_of(&results), ["e1"]);
    assert_eq!(results.hits[0].reason, Reason::Nearest);
    // Documents A and C are tagged `options`. b1 and d1 come through the graph and are left out.
    let results = found_among(&retriever, None, &tagged(&["options"])).await;
    assert_eq!(texts_of(&results), ["a1", "a2", "a3", "c1"]);
    assert_eq!(
        results.hits[3].reason,
        Reason::Concept("Itô's lemma".to_owned())
    );
    // The book and the author match whatever their capitals.
    let of_document_a = DocumentLabels {
        book: Some("pricing options".to_owned()),
        author: Some("SHELDON NATENBERG".to_owned()),
        ..DocumentLabels::default()
    };
    let results = found_among(&retriever, None, &of_document_a).await;
    assert_eq!(texts_of(&results), ["a1", "a2", "a3"]);
    // Every label that is given must fit, and A is not tagged `futures`.
    let not_document_a = DocumentLabels {
        book: Some("Pricing Options".to_owned()),
        ..tagged(&["futures"])
    };
    let results = found_among(&retriever, None, &not_document_a).await;
    assert!(results.hits.is_empty(), "{results}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-retrieval --test integration -- --ignored search::"]
async fn a_returned_paragraph_pulls_in_the_figure_and_the_formula_it_cites() {
    let throwaway = ThrowawayStores::new("search-cited");
    let stores = throwaway.connect().await;
    let mut fixture = Fixture::new();
    let document_a = fixture.document("Document A");
    let document_b = fixture.document("Document B");
    // Document A fills every seed with a chunk, so nothing else in the stores is a seed and the
    // printed label is the only way to the items that are cited.
    fixture.add(Placed::chunk(document_a, "a1", 0.90).citing(&[
        "Figure 13-4",
        "(7.3)",
        "Table 9-9",
    ]));
    for index in 1..RESULTS_PER_QUERY {
        let name = format!("a{}", index + 1);
        fixture.add(Placed::chunk(document_a, &name, 0.90 - 0.01 * index as f32));
    }
    let picture = Path::new("figures/figure-13-4.png");
    fixture.add(Placed::figure(
        document_a,
        "figure of A",
        0.05,
        "Figure 13-4",
        picture,
    ));
    fixture.add(Placed::formula(document_a, "formula of A", 0.04, "(7.3)"));
    fixture.add(Placed::formula(document_b, "formula of B", 0.06, "(7.3)"));
    fixture.label(document_a, tagged(&["options"]));
    fixture.label(document_b, tagged(&["futures"]));
    fixture.store_in(&stores).await;
    let retriever = Retriever {
        embedder: Fixture::embedder(),
        items: stores.items,
        concepts: stores.concepts,
        graph: stores.graph,
    };

    let results = found_among(&retriever, None, &DocumentLabels::default()).await;

    let found: Vec<&str> = results
        .hits
        .iter()
        .map(|hit| hit.item.payload.text.as_str())
        .collect();
    assert_eq!(
        found,
        ["a1", "a2", "a3", "figure of A", "formula of A"],
        "the formula of document B has the label but is in another document, and nothing has the label Table 9-9"
    );
    assert_eq!(
        results.hits[3].reason,
        Reason::Cited {
            by: 1,
            label: "Figure 13-4".to_owned()
        }
    );
    assert_eq!(
        results.hits[4].reason,
        Reason::Cited {
            by: 1,
            label: "(7.3)".to_owned()
        }
    );
    let printed = results.to_string();
    let lines: Vec<&str> = printed.lines().collect();
    assert!(
        lines.contains(&"cited by result 1 as Figure 13-4"),
        "{printed}"
    );
    assert!(lines.contains(&"cited by result 1 as (7.3)"), "{printed}");
    assert!(lines.contains(&"label: (7.3)"), "{printed}");
    assert!(
        lines.contains(&format!("picture: {}", picture.display()).as_str()),
        "{printed}"
    );

    let chunks_only = found_among(
        &retriever,
        Some(ItemKind::Chunk),
        &DocumentLabels::default(),
    )
    .await;
    let found: Vec<&str> = chunks_only
        .hits
        .iter()
        .map(|hit| hit.item.payload.text.as_str())
        .collect();
    assert_eq!(
        found,
        ["a1", "a2", "a3"],
        "nothing is pulled in with --kind"
    );

    // The labels of A keep what A cites, and nothing of B comes in.
    let results = found_among(&retriever, None, &tagged(&["options"])).await;
    assert_eq!(
        texts_of(&results),
        ["a1", "a2", "a3", "figure of A", "formula of A"]
    );
    // The formula of B is nearer and is a formula, so the result is A's only when both the kind and
    // the labels are kept.
    let formulas = found_among(&retriever, Some(ItemKind::Formula), &tagged(&["options"])).await;
    assert_eq!(texts_of(&formulas), ["formula of A"]);
}

fn assert_send<T: Send>(value: T) -> T {
    value
}

// SMELL: this file is over 400 lines. This test and its fixture want a file of their own.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-retrieval --test integration -- --ignored search::"]
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

    // No document is tagged `rates`, so no step runs.
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
