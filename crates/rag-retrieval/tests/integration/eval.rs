//! Scores golden questions against a throwaway collection that holds the three committed
//! chapters, with an embedder that makes up its vectors, so nothing is billed.

use std::collections::BTreeSet;

use rag_core::{ItemKind, ItemStore};
use rag_ingestion::Item;
use rag_retrieval::{GoldenQuestion, Retriever, evaluate, read_golden_questions};

use crate::support::{
    INTUITION_CHAPTER_TITLE, SAMPLE_CHAPTER_TITLE, ThrowawayCollection, WordEmbedder, find_item,
    sample_items, store_samples, workspace_root,
};

/// A question that is the whole stored text of the item, so the item is the nearest one.
fn question_about(item: &Item) -> GoldenQuestion {
    GoldenQuestion {
        text: item.payload.text.clone(),
        document: item.payload.doc_title.clone(),
        page: item.payload.page,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant from docker compose and bills nothing; run with: cargo test -p rag-retrieval --test integration -- --ignored eval::"]
async fn eval_counts_the_golden_questions_found_in_the_top_five() {
    let throwaway = ThrowawayCollection::new("eval");
    let store = ItemStore::connect(throwaway.config()).unwrap();
    let embedder = WordEmbedder::default();
    let items = store_samples(&embedder, &store).await;
    let retriever = Retriever::new(embedder, store);
    let table = find_item(&items, INTUITION_CHAPTER_TITLE, ItemKind::Table, 2);
    let surface = find_item(&items, SAMPLE_CHAPTER_TITLE, ItemKind::Figure, 7);
    let chart = find_item(&items, SAMPLE_CHAPTER_TITLE, ItemKind::Figure, 5);
    let golden = [
        question_about(table),
        question_about(surface),
        GoldenQuestion {
            page: 99,
            ..question_about(chart)
        },
    ];

    let report = evaluate(&golden, &retriever).await.unwrap();

    assert_eq!(report.found(), 2);
    assert_eq!(report.asked(), 3);
    let places: Vec<Option<usize>> = report
        .outcomes
        .iter()
        .map(|outcome| outcome.found_at)
        .collect();
    assert!(
        matches!(places[0], Some(place) if (1..=5).contains(&place)),
        "{places:?}"
    );
    assert!(
        matches!(places[1], Some(place) if (1..=5).contains(&place)),
        "{places:?}"
    );
    assert_eq!(places[2], None);
    let printed = report.to_string();
    assert_eq!(printed.lines().last(), Some("found in the top 5: 2 of 3"));
    let missed: Vec<&str> = printed
        .lines()
        .filter(|line| line.starts_with("missed: "))
        .collect();
    assert_eq!(missed.len(), 1, "{printed}");
    assert!(missed[0].contains(SAMPLE_CHAPTER_TITLE), "{}", missed[0]);
    assert!(missed[0].contains("page 99"), "{}", missed[0]);
}

#[test]
fn every_committed_golden_question_names_a_page_that_holds_items() {
    let golden = read_golden_questions(&workspace_root().join("golden.toml")).unwrap();
    let pages: BTreeSet<(String, u32)> = sample_items()
        .into_iter()
        .map(|item| (item.payload.doc_title, item.payload.page))
        .collect();

    for question in &golden {
        assert!(
            pages.contains(&(question.document.clone(), question.page)),
            "no item is on page {} of {}, so this question can never be found: {}",
            question.page,
            question.document,
            question.text
        );
    }
}
