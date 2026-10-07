//! Names for the collections that a test makes in the real Qdrant for itself, and the removal of
//! such a collection, so that no test touches a real collection.

use std::time::{SystemTime, UNIX_EPOCH};

use qdrant_client::Qdrant;
use rag_core::Config;

pub const COLLECTION_PREFIX: &str = "test-";

/// A name that no other test has and that no real collection has.
pub fn collection_name(test_name: &str) -> String {
    let nanoseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock should be after 1970")
        .as_nanos();
    format!(
        "{COLLECTION_PREFIX}{test_name}-{}-{nanoseconds}",
        std::process::id()
    )
}

/// A client of the Qdrant in the config, for what the stores do not offer: a collection made in
/// the shape of an older program, a look at the indexes, and the removal of a collection.
pub fn qdrant(config: &Config) -> Qdrant {
    Qdrant::from_url(&config.qdrant_url)
        .skip_compatibility_check()
        .build()
        .expect("the Qdrant client should be set up")
}

/// Removes a collection that a test made. It refuses any other name, so it never removes a real
/// collection.
pub async fn remove_collection(config: &Config, collection: &str) {
    assert!(collection.starts_with(COLLECTION_PREFIX));
    qdrant(config)
        .delete_collection(collection)
        .await
        .expect("the throwaway collection should be deleted");
}
