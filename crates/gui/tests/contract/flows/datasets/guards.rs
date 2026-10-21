//! While a build runs no delete starts, and while a delete is on its way no build starts: the
//! buttons of the one are off and say why, and what is pushed anyway sends nothing.

use eframe::egui::accesskit::Role;
use egui_kittest::kittest::NodeT as _;
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{Command, DatasetRequest, DocId, Intent};
use gui::testkit::{self, run_until};
use uuid::Uuid;

use super::{is_at_its_stall, is_build, sent_builds};
use crate::flows::recording;
use crate::flows::{click, delete_buttons, hover_on, is_enabled, node, says, shared, type_into};

const OFF_WHILE_BUILDING: &str = "Nothing can be deleted while a dataset is being built.";
const OFF_WHILE_DELETING: &str = "A delete is on its way. Start when it is done.";

/// Swallows every delete of a document, so it stays on its way for ever, and hands every other
/// command on.
struct Held<H>(H);

impl<H: Handler> Handler for Held<H> {
    async fn serve(&self, command: Command, reply: Reply) {
        match command {
            Command::DeleteDocument { .. } => {}
            other => self.0.serve(other, reply).await,
        }
    }
}

fn a_build_that_runs_turns_every_delete_off() {
    let (mut harness, seen) = recording::open("datasets-running", DEFAULT_WINDOW);
    // The build never ends, so the app is never idle: `click` and `settle` would wait for ever.
    run_until(&mut harness, "the catalogue and the stall", |shared| {
        shared.library.catalogue.ready().is_some() && is_at_its_stall(shared)
    });
    node(&harness, Role::Tab, "Library").click();
    harness.run_ok();

    let deletes = delete_buttons(&harness);
    assert!(!deletes.is_empty());
    for button in &deletes {
        assert!(button.accesskit_node().is_disabled(), "{button:?} is off");
    }
    let first_delete = deletes[0].accesskit_node().label().unwrap_or_default();
    hover_on(&mut harness, Role::Button, &first_delete);
    assert!(says(&harness, OFF_WHILE_BUILDING));
    // The build never ends, so `click` would wait for ever.
    node(&harness, Role::Button, "Quanty Sample Notes").click();
    harness.run_ok();
    assert!(
        is_enabled(&harness, Role::Button, "Edit Quanty Sample Notes"),
        "a build does not hold back the labels of a media"
    );

    harness
        .state_mut()
        .push(Intent::DeleteDocument(DocId(Uuid::from_u128(1))));
    harness
        .state_mut()
        .push(Intent::StartDatasetBuild(DatasetRequest {
            name: "other".to_owned(),
            ..DatasetRequest::default()
        }));
    harness.run_ok();
    assert_eq!(
        seen.count(|command| matches!(command, Command::DeleteDocument { .. })),
        0,
        "no delete is sent while a build runs"
    );
    assert_eq!(
        seen.count(is_build),
        1,
        "no second build is sent while one runs"
    );
}

fn a_delete_on_its_way_turns_the_start_off() {
    let (mut harness, seen) =
        recording::open_with("datasets-ready", DEFAULT_WINDOW, |fake: Fake| Held(fake));
    testkit::settle(&mut harness);
    type_into(&mut harness, "Dataset name", "notes-v3");
    assert!(is_enabled(&harness, Role::Button, "Start build"));

    click(&mut harness, Role::Tab, "Library");
    click(&mut harness, Role::Button, "Option Volatility and Pricing");
    click(&mut harness, Role::Button, "Chapter 1 · Sample Pages");
    click(
        &mut harness,
        Role::Button,
        "Delete document Chapter 1 · Sample Pages",
    );
    // The delete is never answered, so the app is never idle again.
    node(&harness, Role::Button, "Delete document").click();
    harness.run_ok();
    assert!(shared(&harness).library.is_deleting());

    node(&harness, Role::Tab, "Datasets").click();
    harness.run_ok();
    assert!(!is_enabled(&harness, Role::Button, "Start build"));
    hover_on(&mut harness, Role::Button, "Start build");
    assert!(says(&harness, OFF_WHILE_DELETING));
    assert!(sent_builds(&seen).is_empty(), "no build was sent");
}

#[test]
fn a_build_and_a_delete_hold_each_other_off() {
    a_build_that_runs_turns_every_delete_off();
    a_delete_on_its_way_turns_the_start_off();
}
