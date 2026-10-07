//! Asks questions of throwaway stores: one that holds the three committed chapters, and others
//! that are filled by hand with items at known distances from the question, with an embedder that
//! makes up its vectors, so nothing is billed. It also runs the command against stores that are
//! down.

use std::path::Path;
use std::process::Command;

use graph::RelationKind;
use rag_core::{ApiKey, Config, ItemId, ItemKind};
use rag_ingestion::testing::ThrowawayStores;
use rag_retrieval::{MAX_RESULTS_PER_DOCUMENT, RESULTS_PER_QUERY, Reason, Retriever};

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

    let results = retriever.search(&question, None).await.unwrap();

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

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-retrieval --test integration -- --ignored search::"]
async fn a_kind_filter_returns_only_items_of_that_kind() {
    let throwaway = ThrowawayStores::new("search-kind");
    let stores = throwaway.connect().await;
    let embedder = WordEmbedder::default();
    store_samples(&embedder, &stores.items, &stores.concepts).await;
    let retriever = Retriever {
        embedder,
        items: stores.items,
        concepts: stores.concepts,
        graph: stores.graph,
    };

    for kind in ItemKind::ALL {
        let results = retriever
            .search("What is the price of an option?", Some(kind))
            .await
            .unwrap();

        assert!(
            !results.hits.is_empty(),
            "no item of the kind {} was found",
            kind.as_str()
        );
        for hit in &results.hits {
            assert_eq!(hit.item.payload.kind, kind);
        }
    }
}

/// How far the score that Qdrant gives an item may be from the cosine the item was placed at.
const SCORE_ERROR: f32 = 0.001;

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

    let results = retriever.search(QUESTION, None).await.unwrap();

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
    fixture.store_in(&stores).await;
    let retriever = Retriever {
        embedder: Fixture::embedder(),
        items: stores.items,
        concepts: stores.concepts,
        graph: stores.graph,
    };

    let results = retriever.search(QUESTION, None).await.unwrap();

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

    let chunks_only = retriever
        .search(QUESTION, Some(ItemKind::Chunk))
        .await
        .unwrap();
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
}

#[test]
fn an_unknown_kind_is_refused_with_the_list_of_kinds() {
    let output = Command::new(env!("CARGO_BIN_EXE_rag-query"))
        .args(["--kind", "poem", "anything"])
        .output()
        .expect("the rag-query binary should start");

    assert_eq!(output.status.code(), Some(2));
    let refusal = String::from_utf8_lossy(&output.stderr);
    assert!(refusal.contains("poem"), "{refusal}");
    for kind in ItemKind::ALL {
        assert!(refusal.contains(kind.as_str()), "{refusal}");
    }
}

#[test]
fn rag_query_names_the_graph_store_it_cannot_reach() {
    let stamp = format!("unreachable-{}", std::process::id());
    let settings = [
        ("QDRANT_URL", "http://127.0.0.1:1".to_owned()),
        ("FALKORDB_URL", "falkor://127.0.0.1:1".to_owned()),
        ("EMBEDDING_GEMINI_API_KEY", "not-a-real-key".to_owned()),
        ("QDRANT_ITEMS_COLLECTION", format!("test-items-{stamp}")),
        (
            "QDRANT_CONCEPTS_COLLECTION",
            format!("test-concepts-{stamp}"),
        ),
        ("FALKORDB_GRAPH", format!("test-graph-{stamp}")),
    ];
    // The names are typed by hand. A mistyped one would make the command fall back to a real
    // address or a real collection, so read them back the way the command reads them.
    let read_back = Config::from_sources(
        |name| {
            settings
                .iter()
                .find(|(setting, _)| *setting == name)
                .map(|(_, value)| value.clone())
        },
        None,
    )
    .unwrap();
    assert_eq!(read_back.qdrant_url, "http://127.0.0.1:1");
    assert_eq!(read_back.falkordb_url, "falkor://127.0.0.1:1");
    assert_eq!(read_back.items_collection, format!("test-items-{stamp}"));
    assert_eq!(
        read_back.concepts_collection,
        format!("test-concepts-{stamp}")
    );
    assert_eq!(read_back.falkordb_graph, format!("test-graph-{stamp}"));
    // A mistyped name of the key would leave the real key of the `.env` file in reach.
    assert_eq!(
        read_back.gemini_api_key.as_ref().map(ApiKey::expose),
        Some("not-a-real-key")
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rag-query"))
        .arg(QUESTION)
        .envs(settings)
        .output()
        .expect("the rag-query binary should start");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("FalkorDB"), "{stderr}");
    assert!(stderr.contains("falkor://127.0.0.1:1"), "{stderr}");
    assert!(output.stdout.is_empty(), "nothing is printed as a result");
}
