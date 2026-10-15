//! Saved choices must work without parsing unrelated environment settings.
use model_settings::{MODEL_SETTINGS_FILE, ModelSettings};

#[test]
fn saved_selection_precedes_environment_file() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join(MODEL_SETTINGS_FILE);
    let settings = ModelSettings::default();
    settings.save_to(&path).unwrap();
    std::fs::write(folder.path().join(".env"), "INVALID=\"unterminated").unwrap();

    assert_eq!(ModelSettings::load_from(&path).unwrap(), settings);
}
