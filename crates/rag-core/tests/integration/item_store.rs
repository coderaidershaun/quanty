//! Asks the real Qdrant whether a collection is there, makes it, and searches it by kind, because
//! only the real server tells a collection that is missing from a request that failed, refuses a
//! collection that is made twice, and accepts or refuses an index.

use std::time::{SystemTime, UNIX_EPOCH};

use qdrant_client::Qdrant;
use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, VectorParamsBuilder};
use rag_core::{
    Config, DocId, DocumentLabels, EMBEDDING_DIMENSIONS, ItemFilter, ItemId, ItemKind, ItemPayload,
    ItemPoint, ItemStore,
};

use crate::throwaway::{COLLECTION_PREFIX, collection_name, qdrant, remove_collection};

const THROWAWAY_PREFIX: &str = "test-items-";

#[tokio::test]
#[ignore = "needs the local Qdrant from docker compose and bills nothing; run with: cargo test -p rag-core --test integration -- --ignored item_store::"]
async fn a_collection_that_is_missing_is_told_from_one_that_is_there() {
    let nanoseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock should be after 1970")
        .as_nanos();
    let settings = Config::load().expect("the settings should load");
    let config = Config {
        items_collection: format!(
            "{THROWAWAY_PREFIX}exists-{}-{nanoseconds}",
            std::process::id()
        ),
        ..settings
    };
    // The name is set by hand. Without this check, a name that was not set would leave the real
    // collection in the config, and the test would create and delete it.
    assert!(config.items_collection.starts_with(THROWAWAY_PREFIX));
    let store = ItemStore::connect(&config).expect("the item store should open");

    // Twice, so that a call which creates the collection shows: the second call would find it.
    let missing = store.collection_exists().await.unwrap();
    let still_missing = store.collection_exists().await.unwrap();

    // Nothing is asserted between the creation and the deletion, so a failure cannot leave the
    // collection behind.
    store.ensure_collection().await.unwrap();
    let there = store.collection_exists().await;
    Qdrant::from_url(&config.qdrant_url)
        .skip_compatibility_check()
        .build()
        .expect("the Qdrant client should be set up")
        .delete_collection(config.items_collection.as_str())
        .await
        .expect("the throwaway collection should be deleted");

    assert!(!missing, "a collection that was never made is not there");
    assert!(!still_missing, "asking must not make the collection");
    assert!(there.unwrap(), "a collection that was made is there");
}

/// The settings with a collection of items that only this test uses.
fn throwaway_items(test_name: &str) -> Config {
    let settings = Config::load().expect("the settings should load");
    let config = Config {
        items_collection: collection_name(test_name),
        ..settings
    };
    // The name is set by hand. Without this check, a name that was not set would leave the real
    // collection in the config, and the test would write to it and delete it.
    assert!(config.items_collection.starts_with(COLLECTION_PREFIX));
    config
}

fn unit_vector() -> Vec<f32> {
    let mut vector = vec![0.0; EMBEDDING_DIMENSIONS];
    vector[0] = 1.0;
    vector
}

fn point(document: DocId, kind: ItemKind, position: u32) -> ItemPoint {
    ItemPoint {
        id: ItemId::new(document, kind, position),
        vector: unit_vector(),
        payload: ItemPayload {
            doc_id: document,
            doc_title: "A chapter".to_owned(),
            page: 1,
            printed_page: None,
            kind,
            text: format!("item {position}"),
            image_path: None,
            label: None,
            cites: Vec::new(),
            document_labels: DocumentLabels::default(),
        },
    }
}

#[tokio::test]
#[ignore = "needs the local Qdrant from docker compose and bills nothing; run with: cargo test -p rag-core --test integration -- --ignored item_store::"]
async fn two_programs_that_start_together_both_find_the_collection_made() {
    let config = throwaway_items("items-together");
    let first = ItemStore::connect(&config).expect("the item store should open");
    let second = ItemStore::connect(&config).expect("the item store should open");

    // Both look before either one creates, so Qdrant refuses one of the two. Nothing is asserted
    // between the creation and the deletion, so a failure cannot leave the collection behind.
    let (first_made, second_made) =
        tokio::join!(first.ensure_collection(), second.ensure_collection());
    remove_collection(&config, &config.items_collection).await;

    first_made.expect("the first program should find the collection made");
    second_made.expect("the second program should find the collection made");
}

#[tokio::test]
#[ignore = "needs the local Qdrant from docker compose and bills nothing; run with: cargo test -p rag-core --test integration -- --ignored item_store::"]
async fn a_collection_that_holds_items_gets_the_index_on_the_kind_and_is_searched_by_kind() {
    let config = throwaway_items("items-kind-index");
    let name = config.items_collection.as_str();
    let store = ItemStore::connect(&config).expect("the item store should open");
    let document = DocId::from_source_sha256("items-kind-index");
    let chunk = point(document, ItemKind::Chunk, 0);
    let formula = point(document, ItemKind::Formula, 1);
    let only_formulas = ItemFilter {
        kind: Some(ItemKind::Formula),
        documents: None,
    };

    // Nothing is asserted between the creation and the deletion, so a failure cannot leave the
    // collection behind.
    //
    // The collection is made here with no index and filled, as a program made it before the index
    // existed, so that the index is added to a collection that already holds items.
    let vectors = VectorParamsBuilder::new(EMBEDDING_DIMENSIONS as u64, Distance::Cosine);
    let made = qdrant(&config)
        .create_collection(CreateCollectionBuilder::new(name).vectors_config(vectors))
        .await;
    let stored = store.upsert(&[chunk, formula.clone()]).await;
    let indexed = store.ensure_collection().await;
    // A second time, because every start asks for the index and finds it already there.
    let indexed_again = store.ensure_collection().await;
    let found = store.search(unit_vector(), &only_formulas, 10).await;
    let count = store.count().await;
    let info = qdrant(&config).collection_info(name).await;
    remove_collection(&config, name).await;

    made.expect("the collection should be made");
    stored.expect("the items should be stored");
    indexed.expect("a collection that holds items should take the index");
    indexed_again.expect("a collection that has the index should be left as it is");
    let found: Vec<ItemId> = found.unwrap().into_iter().map(|hit| hit.id).collect();
    assert_eq!(found, vec![formula.id], "only the formula is of that kind");
    assert_eq!(count.unwrap(), 2, "the index removes no item");
    let indexes = info.unwrap().result.expect("the collection is described");
    assert!(
        indexes.payload_schema.contains_key("kind"),
        "the kind is indexed: {:?}",
        indexes.payload_schema
    );
}
