//! The Answer and Follow up views in a column as narrow as a phone: nothing runs past the view
//! except inside a sideways scroll, no formula stands on its number, and every suggested question
//! can be read and tapped.

use contract::{AnswerBlock, ItemKind};
use eframe::egui::accesskit::Role;
use eframe::egui::{Pos2, Rect, Vec2};
use egui_kittest::kittest::Queryable as _;
use gui::app::layout::{PHONE_WINDOW, SMALLEST_WINDOW};
use gui::testkit;
use gui::theme::size;

use crate::flows::{Window, click_in, node_in, panels, shared};

#[test]
fn the_answer_fits_a_column_as_narrow_as_a_small_phone() {
    // As tall as it takes for the whole answer to be drawn with no scroll.
    let window = [SMALLEST_WINDOW[0], 3000.0];
    let mut harness = testkit::app("black-scholes", window);
    testkit::settle(&mut harness);
    let answer = panels(window).answer;

    let blocks = &shared(&harness)
        .ask
        .answer
        .ready()
        .expect("an answer")
        .blocks;
    let Some(AnswerBlock::Item(number)) = blocks.last() else {
        panic!("the answer ends with a card");
    };
    let last_card = format!("Result {number}");
    let last = node_in(&harness, Role::Button, &last_card, answer);
    assert!(
        last.is_some(),
        "`{last_card}` is drawn, so the whole answer is"
    );
    assert_no_formula_stands_on_its_number(&harness, answer);
    assert_nothing_runs_past(&harness, answer);
    testkit::save_png(&mut harness, "narrow-answer-360");

    let row = Rect::from_x_y_ranges(f32::NEG_INFINITY..=f32::INFINITY, answer.y_range());
    let results = node_in(&harness, Role::Tab, "Results", row).expect("the Results tab");
    results.scroll_to_me();
    results.click();
    testkit::settle(&mut harness);
    assert_nothing_runs_past(&harness, answer);
    assert_figure_text_stands_under_its_picture(&harness, answer);
    testkit::save_png(&mut harness, "narrow-results-360");
}

/// Every formula card of the answer that has a printed number is drawn in the pane, and the two
/// do not overlap.
fn assert_no_formula_stands_on_its_number(harness: &Window, answer: Rect) {
    let ask = &shared(harness).ask;
    let blocks = &ask.answer.ready().expect("an answer").blocks;
    let cards = blocks.iter().filter_map(|block| match block {
        AnswerBlock::Item(number) => ask.result(*number),
        _ => None,
    });
    for result in cards.filter(|result| result.kind == ItemKind::Formula) {
        let Some(label) = &result.label else {
            continue;
        };
        let formula = node_in(harness, Role::Image, &result.text, answer).expect("the formula");
        let number = node_in(harness, Role::Label, label, answer).expect("its number");
        let both = formula.rect().intersect(number.rect());
        assert!(
            both.width() <= 0.0 || both.height() <= 0.0,
            "the formula {:?} stands on its number `{label}` {:?}",
            formula.rect(),
            number.rect()
        );
    }
}

/// Nothing that the pane draws by itself is wider than the pane. What a sideways scroll area
/// holds is clipped to the area, whose left edge is inside the pane, so it is left out.
fn assert_nothing_runs_past(harness: &Window, answer: Rect) {
    let mut of_the_body = 0;
    for clipped in &harness.output().shapes {
        let clip = clipped.clip_rect;
        let bounds = clipped.shape.visual_bounding_rect();
        let is_the_panes_own =
            answer.expand(0.5).contains_rect(clip) && (clip.left() - answer.left()).abs() <= 0.5;
        if !is_the_panes_own || !bounds.is_finite() || bounds.width() < 0.0 || bounds.height() < 0.0
        {
            continue;
        }
        assert!(
            bounds.left() >= answer.left() - 1.0 && bounds.right() <= answer.right() + 1.0,
            "{:.100} runs past the pane {answer:?}",
            format!("{:?}", clipped.shape)
        );
        if clip.top() > answer.top() + 0.5 {
            of_the_body += 1;
        }
    }
    // The body scrolls up and down, so what it draws is clipped below the header. If the check saw
    // none of it, it left out the whole body and proved nothing.
    assert!(of_the_body > 0, "the check saw nothing of the body");
}

/// The picture of the figure in the list is over its words, and not beside them where a narrow
/// pane would squeeze the words.
fn assert_figure_text_stands_under_its_picture(harness: &Window, answer: Rect) {
    let figure = shared(harness)
        .ask
        .results()
        .iter()
        .find(|result| result.kind == ItemKind::Figure)
        .expect("a figure among the results");
    let card = node_in(
        harness,
        Role::Button,
        &format!("Result {}", figure.number),
        answer,
    )
    .expect("the card of the figure")
    .rect();
    let in_card = |role: Role| {
        harness
            .query_all_by_role(role)
            .filter(|node| card.contains(node.rect().center()))
            .collect::<Vec<_>>()
    };
    let pictures = in_card(Role::Image);
    let picture = pictures.first().expect("the picture of the figure").rect();
    for label in in_card(Role::Label) {
        let top = label.rect().top();
        assert!(
            top < picture.top() - 0.5 || top >= picture.bottom() - 0.5,
            "a label at {:?} stands beside the picture {picture:?}",
            label.rect()
        );
    }
}

#[test]
fn a_phone_shows_each_suggested_question_whole_and_asks_it() {
    let mut harness = testkit::app("black-scholes", PHONE_WINDOW);
    testkit::settle(&mut harness);
    let rects = panels(PHONE_WINDOW);
    let nav = rects.bottom_nav.expect("a phone has a bottom navigation");
    click_in(&mut harness, Role::Tab, "Follow up", nav);

    let answer = shared(&harness)
        .ask
        .answer
        .ready()
        .expect("an answer")
        .clone();
    let panel = rects.follow_up.expand(0.5);
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::from(PHONE_WINDOW)).expand(0.5);
    let rows: Vec<Rect> = answer
        .follow_ups
        .iter()
        .map(|question| node_in(&harness, Role::Button, question, rects.follow_up))
        .map(|row| row.expect("a row for the suggestion").rect())
        .collect();
    for (question, row) in answer.follow_ups.iter().zip(&rows) {
        assert!(
            panel.contains_rect(*row) && screen.contains_rect(*row),
            "`{question}` {row:?}"
        );
        assert!(
            row.height() >= size::CONTROL_MD,
            "`{question}` is {} tall",
            row.height()
        );
    }
    assert!(
        rows.windows(2)
            .all(|pair| pair[0].bottom() <= pair[1].top() + 0.5),
        "the rows do not overlap: {rows:?}"
    );
    testkit::save_png(&mut harness, "phone-follow-up-rows");

    let (longest, row) = answer
        .follow_ups
        .iter()
        .zip(&rows)
        .max_by_key(|(question, _)| question.chars().count())
        .expect("the answer suggests questions");
    assert!(
        row.height() > size::CONTROL_MD,
        "`{longest}` wraps, so it is more than one line tall"
    );
    let generation = shared(&harness).ask.generation;
    click_in(&mut harness, Role::Button, longest, rects.follow_up);
    let ask = &shared(&harness).ask;
    assert_eq!(
        (ask.generation, ask.question.as_str()),
        (generation + 1, longest.as_str())
    );
    let is_answer_view = node_in(&harness, Role::Tab, "Results", rects.answer).is_some();
    assert!(is_answer_view, "a new ask shows the Answer view");
}
