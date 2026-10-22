//! Each work tab keeps its own model choice.

use super::{click, has, node};
use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::testkit;

#[test]
fn each_tab_shows_and_changes_its_own_model() {
    let (mut harness, seen) = super::recording::open("idle", DEFAULT_WINDOW);
    testkit::settle(&mut harness);

    click(&mut harness, Role::ComboBox, "Model");
    click(&mut harness, Role::Button, "Codex · GPT-6 Astra");
    click(&mut harness, Role::Tab, "Ingest");
    assert_eq!(
        node(&harness, Role::ComboBox, "Model").value().as_deref(),
        Some("Codex · GPT-6.1 Sol")
    );
    click(&mut harness, Role::ComboBox, "Model");
    click(&mut harness, Role::Button, "Codex · GPT-6 Luna");
    click(&mut harness, Role::Tab, "Datasets");
    assert_eq!(
        node(&harness, Role::ComboBox, "Model").value().as_deref(),
        Some("Codex · GPT-6.1 Sol")
    );
    click(&mut harness, Role::Tab, "Ask");
    assert_eq!(
        node(&harness, Role::ComboBox, "Model").value().as_deref(),
        Some("Codex · GPT-6 Astra")
    );
    click(&mut harness, Role::Tab, "Library");
    assert!(!has(&harness, Role::ComboBox, "Model"));
    let saved =
        choices::WorkChoices::load_from(&seen.home().join(choices::MODEL_SETTINGS_FILE)).unwrap();
    assert_eq!(saved.ask.model, "gpt-6-astra");
    assert_eq!(saved.ingest.model, "gpt-6-luna");
    assert_eq!(saved.datasets.model, "gpt-6.1-sol");

    let (mut harness, seen) = super::recording::open("idle", DEFAULT_WINDOW);
    std::fs::create_dir(seen.home().join(choices::MODEL_SETTINGS_FILE)).unwrap();
    testkit::settle(&mut harness);
    click(&mut harness, Role::ComboBox, "Model");
    click(&mut harness, Role::Button, "Codex · GPT-6 Astra");
    assert_eq!(
        node(&harness, Role::ComboBox, "Model").value().as_deref(),
        Some("Codex · GPT-6 Astra")
    );
    assert!(
        super::shared(&harness)
            .model_choice_errors
            .contains_key(&choices::WorkKind::Ask)
    );
    assert!(super::says(&harness, "The model choice could not be saved"));
    let painted = harness.output().shapes.iter().find_map(|clipped| {
        if let eframe::egui::Shape::Text(text) = &clipped.shape
            && text
                .galley
                .text()
                .starts_with("The model choice could not be saved")
        {
            Some((clipped.clip_rect, text.visual_bounding_rect()))
        } else {
            None
        }
    });
    let (clip, text) = painted.expect("the save failure is painted below the model picker");
    assert!(
        clip.contains_rect(text),
        "the save failure is clipped: {text:?} outside {clip:?}"
    );
    testkit::save_png(&mut harness, "model-choice-save-failed");
}
