//! Checks that an app that has nothing to do asks for no repaint, also with a sheet open.
//!
//! This is a fitness test because the failure has no symptom except a warm laptop: a panel or a
//! cache that repaints every frame still looks right.

use std::time::Duration;

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::App;
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake;
use gui::contract::{Intent, Tab};
use gui::testkit;

type Window = Harness<'static, App>;

fn repaint_delay(harness: &Window) -> Duration {
    harness.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay
}

/// Presses the first Delete button of a card, which opens the sheet that asks first. False when
/// the page has none.
fn press_the_first_delete_button(harness: &mut Window) -> bool {
    let name = harness
        .query_all_by_role(Role::Button)
        .filter_map(|button| button.accesskit_node().label())
        .find(|name| name.starts_with("Delete media ") || name.starts_with("Delete document "));
    let Some(name) = name else {
        return false;
    };
    harness.get_by_role_and_label(Role::Button, &name).click();
    testkit::settle(harness);
    harness.step();
    harness.step();
    true
}

#[test]
fn an_app_at_rest_asks_for_no_repaint() {
    let mut sheets = 0;
    for scene in fake::scenes().iter().filter(|scene| scene.rests) {
        for tab in Tab::ALL {
            let mut harness = testkit::app(scene.name, DEFAULT_WINDOW);
            harness.state_mut().push(Intent::OpenTab(tab));
            testkit::settle(&mut harness);
            harness.step();
            harness.step();
            assert_eq!(
                repaint_delay(&harness),
                Duration::MAX,
                "the app asked for a repaint in scene `{}` on the {tab:?} tab",
                scene.name
            );
            let name = format!("app-{}-{}", scene.name, format!("{tab:?}").to_lowercase());
            testkit::save_png(&mut harness, &name);

            if tab == Tab::Library && press_the_first_delete_button(&mut harness) {
                assert!(
                    harness
                        .ctx
                        .memory(|memory| memory.top_modal_layer().is_some()),
                    "the sheet did not open in scene `{}`",
                    scene.name
                );
                assert_eq!(
                    repaint_delay(&harness),
                    Duration::MAX,
                    "the app asked for a repaint in scene `{}` with the sheet that asks before a delete open",
                    scene.name
                );
                sheets += 1;
            }
        }
    }
    assert!(
        sheets > 0,
        "no scene had a Delete button, so no open sheet was checked"
    );
}
