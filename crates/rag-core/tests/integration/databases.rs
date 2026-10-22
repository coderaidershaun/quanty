//! The file of saved databases is written to and read from a real folder, and the chosen entry
//! replaces the stores of the settings, with no store running.

use choices::{Chosen, DATABASES_FILE, Database, Databases, DatabasesError, LOCAL_DATABASE};
use rag_core::{ApiKey, Config};
use std::path::{Path, PathBuf};

const STORE_NAMES: [&str; 6] = [
    "QDRANT_URL",
    "QDRANT_API_KEY",
    "FALKORDB_URL",
    "FALKORDB_PASSWORD",
    "FALKORDB_GRAPH",
    "CONTENT_DIR",
];

fn office() -> Database {
    Database {
        name: "Office".into(),
        qdrant_url: "http://127.0.0.1:16334".into(),
        qdrant_api_key: Some(ApiKey::new("office-qdrant-key")),
        falkordb_url: "falkor://127.0.0.1:16379".into(),
        falkordb_password: Some(ApiKey::new("office-falkordb-password")),
        falkordb_graph: "office".into(),
        content_folder: PathBuf::from("/Volumes/quanty-content"),
    }
}

fn local_settings() -> Config {
    let environment = |name: &str| match name {
        "QDRANT_API_KEY" => Some("local-qdrant-key".to_owned()),
        "CONCEPT_CACHE_DIR" => Some("/tmp/concept-cache".to_owned()),
        "DATASET_DIR" => Some("/tmp/datasets".to_owned()),
        "EMBEDDING_GEMINI_API_KEY" => Some("gemini-key".to_owned()),
        _ => None,
    };
    Config::from_sources(environment, None).unwrap()
}

fn set_only(wanted: &'static str) -> impl Fn(&str) -> Option<String> {
    move |name| (name == wanted).then(|| "a value".to_owned())
}

fn saved_with_office() -> Databases {
    let mut databases = Databases::default();
    databases.put(office());
    databases.chosen = Some("Office".into());
    databases
}

fn the_file_gives_back_what_was_saved_and_quotes_nothing_of_a_broken_one(folder: &Path) {
    let path = folder.join(DATABASES_FILE);

    assert_eq!(
        Databases::load_from(&path).unwrap(),
        Databases::default(),
        "no file means nothing chosen"
    );

    // The keys come back too, and the file holds secrets, so only its owner can read it.
    let saved = saved_with_office();
    saved.save_to(&path).unwrap();
    assert_eq!(Databases::load_from(&path).unwrap(), saved);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    assert!(!format!("{saved:?}").contains("office-qdrant-key"));

    std::fs::write(
        &path,
        "{\"chosen\": \"Office\", \"saved\": [{\"name\": 12345-secret}]}",
    )
    .unwrap();
    let error = Databases::load_from(&path).unwrap_err();
    assert!(
        matches!(error, DatabasesError::Parse { line: 1, .. }),
        "{error:?}"
    );
    assert!(!error.to_string().contains("secret"), "{error}");
}

fn the_file_chooses_nothing_local_a_saved_entry_or_a_name_that_is_not_saved() {
    assert_eq!(Databases::default().chosen().unwrap(), Chosen::Nothing);
    assert_eq!(Databases::choosing_local().chosen().unwrap(), Chosen::Local);
    let mut databases = saved_with_office();
    assert_eq!(databases.chosen().unwrap(), Chosen::Saved(&office()));
    databases.chosen = Some("Ghost".into());
    assert!(matches!(
        databases.chosen().unwrap_err(),
        DatabasesError::ChosenNotSaved { name } if name == "Ghost"
    ));
    databases.chosen = Some("office".into());
    assert!(matches!(
        databases.chosen().unwrap_err(),
        DatabasesError::ChosenNotSaved { .. }
    ));

    let mut databases = saved_with_office();
    databases.forget("Office");
    assert_eq!(
        databases,
        Databases::default(),
        "forgetting the chosen entry chooses nothing again"
    );
}

fn the_file_refuses_what_the_app_could_not_use_and_writes_nothing(folder: &Path) {
    let refused_path = folder.join("refused.json");
    for change in [
        (|entry| entry.name = "local".into()) as fn(&mut Database),
        |entry| entry.name = LOCAL_DATABASE.to_uppercase(),
        |entry| entry.qdrant_url = "  ".into(),
        |entry| entry.content_folder = PathBuf::from("quanty/content"),
        |entry| entry.falkordb_password = Some(ApiKey::new("pass\nword")),
    ] {
        let mut entry = office();
        change(&mut entry);
        let mut databases = Databases::default();
        databases.put(entry);
        let error = databases.save_to(&refused_path).unwrap_err();
        assert!(matches!(error, DatabasesError::Invalid { .. }), "{error:?}");
        assert!(!refused_path.exists());
    }
}

fn the_chosen_entry_replaces_exactly_the_six_store_settings() {
    let local = local_settings();
    let switched = local.clone().with_database(&office());
    assert_eq!(switched.qdrant_url, "http://127.0.0.1:16334");
    assert_eq!(
        switched.qdrant_api_key.as_ref().map(ApiKey::expose),
        Some("office-qdrant-key")
    );
    assert_eq!(switched.falkordb_url, "falkor://127.0.0.1:16379");
    assert_eq!(
        switched.falkordb_password.as_ref().map(ApiKey::expose),
        Some("office-falkordb-password")
    );
    assert_eq!(switched.falkordb_graph, "office");
    assert_eq!(
        switched.content_folder,
        PathBuf::from("/Volumes/quanty-content")
    );
    assert_eq!(switched.items_collection, local.items_collection);
    assert_eq!(switched.concepts_collection, local.concepts_collection);
    assert_eq!(switched.concept_cache_folder, local.concept_cache_folder);
    assert_eq!(switched.concept_decision_log, local.concept_decision_log);
    assert_eq!(switched.dataset_folder, local.dataset_folder);
    assert_eq!(switched.dataset_cache_folder, local.dataset_cache_folder);
    assert_eq!(
        switched.gemini_api_key.as_ref().map(ApiKey::expose),
        Some("gemini-key")
    );
    assert!(switched.jev_api_key.is_none());

    // Local is what the settings say of the stores, so it switches back.
    let local_entry = local.local_database();
    assert_eq!(local_entry.name, LOCAL_DATABASE);
    assert!(local_entry.is_local());
    assert_eq!(
        local_entry.qdrant_api_key.as_ref().map(ApiKey::expose),
        Some("local-qdrant-key")
    );
    let back = switched.with_database(&local_entry);
    assert_eq!(format!("{back:?}"), format!("{local:?}"));
    assert_eq!(
        back.qdrant_api_key.as_ref().map(ApiKey::expose),
        Some("local-qdrant-key")
    );

    // Only a saved entry is applied; Nothing and Local leave the settings alone.
    let applied = local.clone().with_chosen(&saved_with_office()).unwrap();
    assert_eq!(applied.falkordb_graph, "office");
    for unchanged in [Databases::default(), Databases::choosing_local()] {
        let config = local.clone().with_chosen(&unchanged).unwrap();
        assert_eq!(format!("{config:?}"), format!("{local:?}"));
    }
    let mut ghost = saved_with_office();
    ghost.chosen = Some("Ghost".into());
    assert!(local.clone().with_chosen(&ghost).is_err());
}

fn the_environment_wins_when_it_names_any_of_the_six_store_settings() {
    assert!(!Config::environment_names_a_store(|_| None));
    assert!(!Config::environment_names_a_store(|_| Some(
        "   ".to_owned()
    )));
    assert!(!Config::environment_names_a_store(set_only(
        "QDRANT_ITEMS_COLLECTION"
    )));
    for name in STORE_NAMES {
        assert!(Config::environment_names_a_store(set_only(name)), "{name}");
    }
}

#[test]
fn the_chosen_database_round_trips_through_its_file_and_replaces_only_the_stores() {
    let folder = tempfile::tempdir().unwrap();
    the_file_gives_back_what_was_saved_and_quotes_nothing_of_a_broken_one(folder.path());
    the_file_chooses_nothing_local_a_saved_entry_or_a_name_that_is_not_saved();
    the_file_refuses_what_the_app_could_not_use_and_writes_nothing(folder.path());
    the_chosen_entry_replaces_exactly_the_six_store_settings();
    the_environment_wins_when_it_names_any_of_the_six_store_settings();
}
