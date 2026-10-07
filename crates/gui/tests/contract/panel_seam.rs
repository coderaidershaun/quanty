//! Checks how a panel and the app hand things to each other: the panel gets a read-only view of the state,
//! answers with intents, and the test kit carries a click all the way to one.

use egui_kittest::kittest::Queryable;
use gui::contract::Intent;
use gui::media::rich_text::{self, Clicked, RichText};
use gui::testkit::{self, sample};
use gui::theme::TextRole;

#[test]
fn a_panel_gives_its_intents_and_never_changes_what_it_reads() {
    let shared = testkit::answered(
        sample::search_reply(),
        sample::concept_graph(),
        sample::answer(),
    );
    let before = shared.clone();
    let mut harness = testkit::panel([986.0, 549.0], shared, |ui, cx| {
        let text = RichText::new(
            "The value of the option satisfies an equation.",
            TextRole::Body,
        )
        .cites(&[1, 2]);
        match rich_text::show(ui, cx.media, &text) {
            Some(Clicked::Citation(number)) => cx.intents.push(Intent::SelectResult(number)),
            Some(Clicked::CopyText(source)) => cx.intents.push(Intent::CopyText(source)),
            None => {}
        }
    });

    harness
        .get_all_by_label("Citation 2")
        .next()
        .expect("a chip for the second citation")
        .click();
    harness.run();

    assert_eq!(harness.state().intents, vec![Intent::SelectResult(2)]);
    assert_eq!(
        harness.state().shared,
        before,
        "a panel reads the state and never changes it"
    );
    testkit::save_png(&mut harness, "stub-panel");
}
