//! Checks how a panel and the app hand things to each other: the panel gets a read-only view of
//! the state, answers with intents, and the test kit carries a click all the way to one.

use std::cell::RefCell;
use std::rc::Rc;

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

    // A panel may ask egui to lay the frame out a second time. The click must still give its
    // intent once: the second pass of a frame gets no input.
    let passes = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&passes);
    let mut harness = testkit::panel([986.0, 549.0], before, move |ui, cx| {
        let text = RichText::new("The value of the option.", TextRole::Body).cites(&[2]);
        let clicked = matches!(
            rich_text::show(ui, cx.media, &text),
            Some(Clicked::Citation(2))
        );
        if clicked {
            cx.intents.push(Intent::SelectResult(2));
            ui.ctx()
                .request_discard("the stand-in panel lays the frame out again");
        }
        seen.borrow_mut()
            .push((ui.ctx().cumulative_frame_nr(), clicked));
    });

    harness
        .get_all_by_label("Citation 2")
        .next()
        .expect("a chip for the second citation")
        .click();
    harness.run();

    let passes = passes.borrow();
    let clicked_frames: Vec<u64> = passes
        .iter()
        .filter(|(_, clicked)| *clicked)
        .map(|(frame, _)| *frame)
        .collect();
    assert_eq!(clicked_frames.len(), 1, "one pass sees the click");
    let passes_of_that_frame = passes
        .iter()
        .filter(|(frame, _)| *frame == clicked_frames[0])
        .count();
    assert_eq!(
        passes_of_that_frame, 2,
        "the frame of the click is laid out twice"
    );
    assert_eq!(
        harness.state().intents,
        vec![Intent::SelectResult(2)],
        "a frame that is laid out twice gives the intent of a click once"
    );
}
