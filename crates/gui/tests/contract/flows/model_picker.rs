//! Ask, Ingest and Datasets each show and change their own model in the top bar, and Library shows
//! none.

use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::testkit;

use super::{click, has, node};

#[test]
fn each_tab_shows_and_changes_its_own_model() {
    let mut harness = testkit::app("idle", DEFAULT_WINDOW);
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
}
