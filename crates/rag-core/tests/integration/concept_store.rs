//! Stores a concept in the real Qdrant and reads it back, because only the real server shows that
//! the payload a write sends is the payload a search can read.

use rag_core::{ConceptHit, ConceptId, ConceptPoint, ConceptStore, Config, EMBEDDING_DIMENSIONS};

use crate::throwaway::{COLLECTION_PREFIX, collection_name, remove_collection};

#[tokio::test]
#[ignore = "needs the local Qdrant from docker compose and bills nothing; run with: cargo test -p rag-core --test integration -- --ignored concept_store::"]
async fn a_stored_concept_is_found_with_its_name_and_the_aliases_that_were_set_last() {
    let settings = Config::load().expect("the settings should load");
    let config = Config {
        concepts_collection: collection_name("concepts-read-back"),
        ..settings
    };
    // The name is set by hand. Without this check, a name that was not set would leave the real
    // collection in the config, and the test would write to it and delete it.
    assert!(config.concepts_collection.starts_with(COLLECTION_PREFIX));
    let store = ConceptStore::connect(&config).expect("the concept store should open");
    let mut vector = vec![0.0; EMBEDDING_DIMENSIONS];
    vector[0] = 1.0;
    let concept = ConceptPoint {
        id: ConceptId::random(),
        vector: vector.clone(),
        name: "Itô's lemma".to_owned(),
        aliases: vec!["Itô's formula".to_owned()],
    };
    let new_aliases = ["the Itô rule".to_owned(), "Itô–Doeblin formula".to_owned()];

    // Nothing is asserted between the creation and the deletion, so a failure cannot leave the
    // collection behind.
    let made = store.ensure_collection().await;
    let stored = store.upsert(&concept).await;
    let found = store.nearest(vector.clone(), 1).await;
    let renamed = store.set_aliases(concept.id, &new_aliases).await;
    let found_again = store.nearest(vector, 1).await;
    remove_collection(&config, &config.concepts_collection).await;

    made.unwrap();
    stored.unwrap();
    renamed.unwrap();
    let hit = |aliases: &[String]| ConceptHit {
        id: concept.id,
        score: 1.0,
        name: concept.name.clone(),
        aliases: aliases.to_vec(),
    };
    assert_eq!(found.unwrap(), vec![hit(&concept.aliases)]);
    assert_eq!(found_again.unwrap(), vec![hit(&new_aliases)]);
}
