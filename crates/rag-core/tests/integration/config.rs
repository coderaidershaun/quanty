//! Reads the settings the way a person sets them: the environment first, then the `.env` file,
//! then the local defaults.

use std::collections::HashMap;
use std::error::Error;

use rag_core::Config;

fn environment_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
    let variables: HashMap<String, String> = pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    move |name| variables.get(name).cloned()
}

fn chain_of(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(&format!(" / {cause}"));
        source = cause.source();
    }
    text
}

#[test]
fn environment_wins_over_dotenv_and_stores_fall_back_to_localhost() {
    let folder = tempfile::tempdir().unwrap();
    let dotenv = folder.path().join(".env");
    std::fs::write(
        &dotenv,
        "QDRANT_URL=http://from-dotenv:6334\n\
         FALKORDB_URL=falkor://from-dotenv:6379\n\
         EMBEDDING_GEMINI_API_KEY=not-a-real-key\n\
         QDRANT_ITEMS_COLLECTION=   \n",
    )
    .unwrap();

    let config = Config::from_sources(
        environment_of(&[("QDRANT_URL", "http://from-environment:6334")]),
        Some(&dotenv),
    )
    .unwrap();
    assert_eq!(config.qdrant_url, "http://from-environment:6334");
    assert_eq!(config.falkordb_url, "falkor://from-dotenv:6379");
    assert_eq!(
        config.gemini_api_key.as_ref().map(|key| key.expose()),
        Some("not-a-real-key")
    );
    assert_eq!(config.items_collection, "items", "a blank value is unset");
    assert!(
        !format!("{config:?}").contains("not-a-real-key"),
        "the debug text of a config must not show the key"
    );

    let config = Config::from_sources(environment_of(&[]), None).unwrap();
    assert_eq!(config.qdrant_url, "http://localhost:6334");
    assert_eq!(config.falkordb_url, "falkor://localhost:6379");
    assert_eq!(config.items_collection, "items");
    assert!(config.gemini_api_key.is_none());

    let config = Config::from_sources(
        environment_of(&[("QDRANT_ITEMS_COLLECTION", "test-items-elsewhere")]),
        Some(&folder.path().join("missing.env")),
    )
    .unwrap();
    assert_eq!(config.items_collection, "test-items-elsewhere");

    std::fs::write(&dotenv, "EMBEDDING_GEMINI_API_KEY not-a-real-key\n").unwrap();
    let error = Config::from_sources(environment_of(&[]), Some(&dotenv)).unwrap_err();
    let message = chain_of(&error);
    assert!(message.contains(&dotenv.display().to_string()), "{message}");
    assert!(
        !message.contains("not-a-real-key"),
        "an error about the file must not repeat its contents: {message}"
    );
}
