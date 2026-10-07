//! Checks that an app that has nothing to do asks for no repaint.
//!
//! This is a fitness test because the failure has no symptom except a warm laptop: a panel or a
//! cache that repaints every frame still looks right.

use std::time::Duration;

use eframe::egui;
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake;
use gui::contract::{Intent, Tab};
use gui::testkit;

#[test]
fn an_app_at_rest_asks_for_no_repaint() {
    for scene in fake::scenes().iter().filter(|scene| scene.rests) {
        for tab in Tab::ALL {
            let mut harness = testkit::app(scene.name, DEFAULT_WINDOW);
            harness.state_mut().push(Intent::OpenTab(tab));
            testkit::settle(&mut harness);
            harness.step();
            harness.step();
            let delay = harness.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
            assert_eq!(
                delay,
                Duration::MAX,
                "the app asked for a repaint in scene `{}` on the {tab:?} tab",
                scene.name
            );
            let name = format!("app-{}-{}", scene.name, format!("{tab:?}").to_lowercase());
            testkit::save_png(&mut harness, &name);
        }
    }
}
