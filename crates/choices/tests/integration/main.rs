//! Saved choices, one for each kind of work, must load from old and new files.
use choices::{MODEL_SETTINGS_FILE, ModelSettings, WorkChoices, WorkKind};

#[test]
fn each_kind_of_work_keeps_its_own_choice() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join(MODEL_SETTINGS_FILE);
    let mut saved = WorkChoices::default();
    saved.ask.select("haiku");
    saved.ingest.select("gpt-6-sol");
    saved.ingest.thinking = "high".into();
    saved.save_to(&path).unwrap();

    let loaded = WorkChoices::load_from(&path).unwrap();

    for work in WorkKind::ALL {
        assert_eq!(loaded.of(work), saved.of(work));
    }
    assert_ne!(loaded.of(WorkKind::Ask), loaded.of(WorkKind::Ingest));
    assert_ne!(loaded.of(WorkKind::Ingest), loaded.of(WorkKind::Datasets));
    assert_ne!(loaded.of(WorkKind::Ask), loaded.of(WorkKind::Datasets));
}

#[test]
fn a_file_saved_before_the_split_gives_its_choice_to_every_kind_of_work() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join(MODEL_SETTINGS_FILE);
    std::fs::write(
        &path,
        r#"{"provider":"claude","model":"opus","thinking":"high"}"#,
    )
    .unwrap();

    let choices = WorkChoices::load_from(&path).unwrap();

    let before = ModelSettings {
        provider: "claude".into(),
        model: "opus".into(),
        thinking: "high".into(),
    };
    for work in WorkKind::ALL {
        assert_eq!(choices.of(work), &before);
    }
    choices.save_to(&path).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for key in ["ask", "ingest", "datasets"] {
        assert!(saved.get(key).is_some(), "the saved file has no {key}");
    }
    assert_eq!(WorkChoices::load_from(&path).unwrap(), choices);
}
