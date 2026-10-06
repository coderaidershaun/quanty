//! Asks questions of a throwaway collection that holds the three committed chapters, with an
//! embedder that makes up its vectors, so nothing is billed.

use std::process::Command;

use rag_core::{ItemKind, ItemStore};
use rag_retrieval::Retriever;

use crate::support::{
    self, SAMPLE_CHAPTER_TITLE, ThrowawayCollection, WordEmbedder, find_item, store_samples,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant from docker compose and bills nothing; run with: cargo test -p rag-retrieval --test integration -- --ignored search::"]
async fn a_question_finds_its_item_and_prints_title_page_kind_text_and_picture() {
    let throwaway = ThrowawayCollection::new("search");
    let store = ItemStore::connect(throwaway.config()).unwrap();
    let embedder = WordEmbedder::default();
    let items = store_samples(&embedder, &store).await;
    let target = find_item(&items, SAMPLE_CHAPTER_TITLE, ItemKind::Figure, 5);
    let question = target.payload.text.clone();
    let retriever = Retriever::new(embedder.clone(), store);

    let results = retriever.search(&question, None, 3).await.unwrap();

    assert_eq!(embedder.questions(), vec![question.clone()]);
    assert_eq!(results.hits.len(), 3);
    assert!(
        results
            .hits
            .windows(2)
            .all(|pair| pair[0].score >= pair[1].score),
        "the scores should not rise"
    );
    let first = &results.hits[0];
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
#[ignore = "needs the local Qdrant from docker compose and bills nothing; run with: cargo test -p rag-retrieval --test integration -- --ignored search::"]
async fn a_kind_filter_returns_only_items_of_that_kind() {
    let throwaway = ThrowawayCollection::new("search-kind");
    let store = ItemStore::connect(throwaway.config()).unwrap();
    let embedder = WordEmbedder::default();
    store_samples(&embedder, &store).await;
    let retriever = Retriever::new(embedder, store);

    for kind in ItemKind::ALL {
        let results = retriever
            .search("What is the price of an option?", Some(kind), 5)
            .await
            .unwrap();

        assert!(
            !results.hits.is_empty(),
            "no item of the kind {} was found",
            kind.as_str()
        );
        for hit in &results.hits {
            assert_eq!(hit.payload.kind, kind);
        }
    }
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
