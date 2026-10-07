//! Asks the real Qdrant whether a collection is there, because only the real server tells a
//! collection that is missing from a request that failed.

use std::time::{SystemTime, UNIX_EPOCH};

use qdrant_client::Qdrant;
use rag_core::{Config, ItemStore};

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
