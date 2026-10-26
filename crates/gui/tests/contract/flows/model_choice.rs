//! Each work tab keeps its own model choice, and a phone reaches it through one button.

use super::{click, has, node, node_in, shared};
use choices::WorkKind;
use contract::{Command, Tab};
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Pos2, Rect, Vec2};
use gui::app::layout::{DEFAULT_WINDOW, PHONE_WINDOW};
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

    for model in ["gpt-6.1-sol", "sonnet"] {
        let home = tempfile::tempdir().unwrap();
        let fake = backend::fake::Fake::scene("idle", home.path())
            .unwrap()
            .instant();
        let mut facts = contract::StartupFacts {
            anthropic_api_key_set: true,
            ..Default::default()
        };
        facts.model_choices.ingest.select(model);
        let mut harness = testkit::app_on(fake, facts, Vec::new(), DEFAULT_WINDOW);
        testkit::settle(&mut harness);
        let health = &super::shared(&harness).health;
        assert!(health.pending.is_none());
        let state = health.of(contract::Service::Claude);
        if model == "sonnet" {
            assert!(
                matches!(state, contract::ServiceState::Down(failure) if failure.kind == contract::FailureKind::ClaudeApiKeySet)
            );
        } else {
            assert!(
                matches!(state, contract::ServiceState::Up { .. }),
                "{state:?}"
            );
        }
    }
}

/// How a test puts the sheet of model choices away.
enum Close {
    Done,
    Escape,
}

#[test]
fn a_phone_keeps_the_model_choices_behind_one_button() {
    let (mut harness, seen) = super::recording::open("idle", PHONE_WINDOW);
    testkit::settle(&mut harness);
    let window = Rect::from_min_size(Pos2::ZERO, Vec2::from(PHONE_WINDOW));
    assert!(
        !has(&harness, Role::ComboBox, "Model"),
        "the dropdowns are not in the bar of a phone"
    );

    for (tab, label, work, close) in [
        (Tab::Ask, "Ask", WorkKind::Ask, Close::Done),
        (Tab::Ingest, "Ingest", WorkKind::Ingest, Close::Done),
        (Tab::Datasets, "Datasets", WorkKind::Datasets, Close::Escape),
    ] {
        click(&mut harness, Role::Tab, label);
        click(&mut harness, Role::Button, "Model choices");
        for dropdown in ["Model", "Thinking level"] {
            let rect = node(&harness, Role::ComboBox, dropdown).rect();
            assert!(window.contains_rect(rect), "{tab:?}: `{dropdown}` {rect:?}");
        }
        if tab == Tab::Ask {
            testkit::save_png(&mut harness, "phone-model-choices");
        }

        let saves = |seen: &super::recording::Seen| {
            seen.count(|command| {
                matches!(command, Command::SaveModelChoice { work: saved, .. } if *saved == work)
            })
        };
        let before = saves(&seen);
        click(&mut harness, Role::ComboBox, "Model");
        click(&mut harness, Role::Button, "Codex · GPT-6 Astra");
        assert_eq!(saves(&seen), before + 1, "{tab:?}: one choice, one save");
        assert!(
            has(&harness, Role::ComboBox, "Model"),
            "{tab:?}: the sheet stays open after a choice"
        );

        match close {
            Close::Done => click(&mut harness, Role::Button, "Done"),
            Close::Escape => {
                harness.key_press(Key::Escape);
                testkit::settle(&mut harness);
            }
        }
        assert!(
            !has(&harness, Role::ComboBox, "Model"),
            "{tab:?}: the sheet is closed"
        );
        assert_eq!(shared(&harness).tab, tab);
    }

    click(&mut harness, Role::Tab, "Library");
    assert!(!has(&harness, Role::Button, "Model choices"));

    let (mut harness, seen) = super::recording::open("idle", PHONE_WINDOW);
    std::fs::create_dir(seen.home().join(choices::MODEL_SETTINGS_FILE)).unwrap();
    testkit::settle(&mut harness);
    click(&mut harness, Role::Button, "Model choices");
    click(&mut harness, Role::ComboBox, "Model");
    click(&mut harness, Role::Button, "Codex · GPT-6 Astra");
    let hint = &shared(&harness).model_choice_errors[&WorkKind::Ask].hint;
    assert!(
        has(&harness, Role::ComboBox, "Model")
            && node_in(&harness, Role::Label, hint, window).is_some(),
        "a touch screen cannot hover, so the open sheet shows `{hint}`"
    );
}
