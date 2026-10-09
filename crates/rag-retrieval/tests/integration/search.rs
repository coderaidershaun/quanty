//! Asks questions of throwaway stores that hold the three committed chapters or items placed by
//! hand, with an embedder that makes up its vectors, so nothing is billed.

use std::collections::BTreeSet;
use std::path::Path;

use graph::RelationKind;
use rag_core::{Category, DocumentLabels, ItemId, ItemKind, LabelFilter, MediaLabels};
use rag_ingestion::testing::ThrowawayStores;
use rag_retrieval::{MAX_RESULTS_PER_DOCUMENT, RESULTS_PER_QUERY, Reason, Retriever};

use crate::support::{
    self, Fixture, Placed, SAMPLE_CHAPTER_TITLE, SCORE_ERROR, WordEmbedder, find_item, found_among,
    store_samples, tagged, texts_of, wanting,
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
        .search(&question, None, &LabelFilter::default())
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

    let used: Vec<_> = results.usage.models().collect();
    let [("gemini-embedding-2", embedded)] = used[..] else {
        panic!("expected the embedding of the question alone, got {used:?}");
    };
    assert!(embedded.estimated && embedded.tokens.input_tokens > 0);
    let last_lines: Vec<&str> = printed.lines().rev().take(3).collect();
    let [cost, tokens, blank] = last_lines[..] else {
        panic!("{printed}");
    };
    assert_eq!(blank, "");
    assert_eq!(
        tokens,
        format!(
            "tokens of gemini-embedding-2: {} in, 0 out, 0 cache read, 0 cache write (estimated)",
            embedded.tokens.input_tokens
        )
    );
    assert_eq!(cost, "cost: under $0.01");
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
    // Document A is of a paper with two authors and the media tag `options`; C has `options` as
    // its own tag.
    let mut of_a_paper = DocumentLabels::default();
    of_a_paper.take_media(
        "Pricing Options",
        &MediaLabels {
            category: Category::Paper,
            authors: vec!["Sheldon Natenberg".to_owned(), "Euan Sinclair".to_owned()],
            tags: BTreeSet::from(["options".parse().unwrap()]),
        },
    );
    fixture.label(document_a, of_a_paper);
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

    let results = found_among(&retriever, None, &LabelFilter::default()).await;

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
    let results = found_among(&retriever, None, &wanting(&["rates"])).await;
    assert_eq!(texts_of(&results), ["e1"]);
    assert_eq!(results.hits[0].reason, Reason::Nearest);
    // A has `options` as a media tag and C as its own tag. b1 and d1 come through the graph and
    // are left out.
    let results = found_among(&retriever, None, &wanting(&["options"])).await;
    assert_eq!(texts_of(&results), ["a1", "a2", "a3", "c1"]);
    assert_eq!(
        results.hits[3].reason,
        Reason::Concept("Itô's lemma".to_owned())
    );
    let of_document_a = LabelFilter {
        media: Some("pricing options".to_owned()),
        author: Some("SHELDON NATENBERG".to_owned()),
        ..LabelFilter::default()
    };
    let results = found_among(&retriever, None, &of_document_a).await;
    assert_eq!(texts_of(&results), ["a1", "a2", "a3"]);
    // The second author of A fits as well as the first.
    let by_the_second_author = LabelFilter {
        author: Some("EUAN SINCLAIR".to_owned()),
        ..LabelFilter::default()
    };
    let results = found_among(&retriever, None, &by_the_second_author).await;
    assert_eq!(texts_of(&results), ["a1", "a2", "a3"]);
    let of_category = |category| LabelFilter {
        category: Some(category),
        ..LabelFilter::default()
    };
    let results = found_among(&retriever, None, &of_category(Category::Paper)).await;
    assert_eq!(texts_of(&results), ["a1", "a2", "a3"]);
    let results = found_among(&retriever, None, &of_category(Category::Other)).await;
    assert!(results.hits.is_empty(), "{results}");
    // Every label that is given must fit, and A has no tag `futures`.
    let not_document_a = LabelFilter {
        media: Some("Pricing Options".to_owned()),
        ..wanting(&["futures"])
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

    let results = found_among(&retriever, None, &LabelFilter::default()).await;

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

    let chunks_only = found_among(&retriever, Some(ItemKind::Chunk), &LabelFilter::default()).await;
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
    let results = found_among(&retriever, None, &wanting(&["options"])).await;
    assert_eq!(
        texts_of(&results),
        ["a1", "a2", "a3", "figure of A", "formula of A"]
    );
    // The formula of B is nearer and is a formula, so the result is A's only when both the kind and
    // the labels are kept.
    let formulas = found_among(&retriever, Some(ItemKind::Formula), &wanting(&["options"])).await;
    assert_eq!(texts_of(&formulas), ["formula of A"]);
}
