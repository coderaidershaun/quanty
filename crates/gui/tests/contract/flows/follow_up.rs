//! A follow-up starts a new ask with the same mode and filters, a plain key stays in the question
//! box, and every way to copy reaches the clipboard.

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::contract::AnswerBlock;
use gui::testkit;

use super::{Window, click, has, is_open_tab, node, press, shared};

/// What the app asked the window to copy in the last frame that ran.
fn copied(harness: &Window) -> Vec<String> {
    harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Runs the frames of the events already queued, and returns what they copied. The copy is in
/// the output of the last frame only, so no frame may run after the one that asks for it.
fn copied_by(harness: &mut Window, act: impl FnOnce(&mut Window)) -> Vec<String> {
    act(harness);
    harness.step();
    let texts = copied(harness);
    testkit::settle(harness);
    texts
}

fn question_box(harness: &Window) -> Option<String> {
    node(harness, Role::TextInput, "Question").value()
}

/// Checks that exactly one new ask was made since `generation`, with this question.
fn assert_new_ask(harness: &Window, generation: u64, question: &str) {
    let ask = &shared(harness).ask;
    assert_eq!(ask.generation, generation + 1, "one new ask was made");
    assert_eq!(ask.question, question);
}

#[test]
fn a_follow_up_starts_a_new_ask_and_copies_reach_the_clipboard() {
    let mut harness = testkit::app("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);

    // The Books list comes from the library, and the ask carries the book that was chosen.
    let books: Vec<String> = shared(&harness)
        .library
        .catalogue
        .ready()
        .map(|catalogue| {
            catalogue
                .books
                .iter()
                .filter_map(|b| b.title.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(!books.is_empty(), "the library has books");
    click(&mut harness, Role::ComboBox, "Books");
    assert!(has(&harness, Role::Button, "All books"));
    for book in &books {
        assert!(
            has(&harness, Role::Button, book),
            "`{book}` is a row of Books"
        );
    }
    click(&mut harness, Role::Button, &books[0]);
    let before = shared(&harness).ask.generation;
    let question = shared(&harness).ask.question.clone();
    assert_eq!(
        question_box(&harness).as_deref(),
        Some(question.as_str()),
        "the bar holds the question of the ask on screen"
    );
    click(&mut harness, Role::Button, "Ask");
    assert_new_ask(&harness, before, &question);
    assert_eq!(shared(&harness).ask.filters.book.as_ref(), Some(&books[0]));

    // A suggestion chip asks its question again with the same mode and filters.
    click(&mut harness, Role::Tab, "Results");
    let follow_up = shared(&harness)
        .ask
        .answer
        .ready()
        .and_then(|answer| answer.follow_ups.first().cloned())
        .expect("the answer suggests a follow-up");
    let (mode, filters) = {
        let ask = &shared(&harness).ask;
        (ask.mode, ask.filters.clone())
    };
    let before = shared(&harness).ask.generation;
    click(&mut harness, Role::Button, &follow_up);
    assert_new_ask(&harness, before, &follow_up);
    assert_eq!(
        (shared(&harness).ask.mode, &shared(&harness).ask.filters),
        (mode, &filters)
    );
    assert_eq!(question_box(&harness).as_deref(), Some(follow_up.as_str()));
    assert!(
        is_open_tab(&harness, "Answer"),
        "a new ask opens the Answer tab again"
    );

    // A typed follow-up.
    let typed = "What does gamma measure?";
    node(&harness, Role::TextInput, "Follow-up question").click();
    node(&harness, Role::TextInput, "Follow-up question").type_text(typed);
    harness.run_ok();
    let before = shared(&harness).ask.generation;
    click(&mut harness, Role::Button, "Ask follow-up");
    assert_new_ask(&harness, before, typed);
    assert_eq!(
        node(&harness, Role::TextInput, "Follow-up question")
            .value()
            .as_deref(),
        Some(""),
        "the follow-up box is empty again"
    );

    // A slash typed in the question box stays in the box, and a slash anywhere else moves the
    // caret to it.
    click(&mut harness, Role::TextInput, "Question");
    let cue = shared(&harness).cues.focus_ask_bar;
    press(&mut harness, egui::Modifiers::NONE, egui::Key::Slash);
    assert_eq!(
        shared(&harness).cues.focus_ask_bar,
        cue,
        "a slash in the box is text"
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.step();
    node(&harness, Role::TextInput, "Question").type_text("a/b");
    testkit::settle(&mut harness);
    assert_eq!(question_box(&harness).as_deref(), Some("a/b"));
    click(&mut harness, Role::Button, "Result 1");
    assert!(
        !node(&harness, Role::TextInput, "Question")
            .accesskit_node()
            .is_focused()
    );
    press(&mut harness, egui::Modifiers::NONE, egui::Key::Slash);
    assert_eq!(shared(&harness).cues.focus_ask_bar, cue + 1);
    assert!(
        node(&harness, Role::TextInput, "Question")
            .accesskit_node()
            .is_focused()
    );

    copies_reach_the_clipboard(&mut harness);
}

fn copies_reach_the_clipboard(harness: &mut Window) {
    let markdown = shared(harness)
        .ask
        .answer_markdown()
        .expect("an answer to share");
    let texts = copied_by(harness, |harness| {
        node(harness, Role::Button, "Share").click()
    });
    assert_eq!(
        texts,
        std::slice::from_ref(&markdown),
        "Share copies the answer as Markdown"
    );

    let texts = copied_by(harness, |harness| {
        // One frame that holds the key, so that the output of the last frame is the one with
        // the copy in it.
        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::S,
            physical_key: Some(egui::Key::S),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        });
    });
    assert_eq!(texts, [markdown], "⇧⌘S copies the same");

    let first_formula = shared(harness)
        .ask
        .results()
        .iter()
        .find(|result| result.kind == gui::contract::ItemKind::Formula)
        .map(|result| result.text.clone())
        .expect("a formula result");
    let texts = copied_by(harness, |harness| {
        node(harness, Role::Button, "Copy LaTeX").click()
    });
    assert_eq!(
        texts,
        [first_formula],
        "Copy LaTeX copies the formula as it is stored"
    );

    let paragraph = shared(harness)
        .ask
        .answer
        .ready()
        .and_then(|answer| {
            answer.blocks.iter().find_map(|block| match block {
                AnswerBlock::Paragraph { text, .. }
                    if harness.query_all_by_label(text).next().is_some() =>
                {
                    Some(text.clone())
                }
                _ => None,
            })
        })
        .expect("a paragraph with no marks, drawn as it is stored");
    node(harness, Role::Label, &paragraph).click_secondary();
    harness.run_ok();
    let texts = copied_by(harness, |harness| {
        node(harness, Role::Button, "Copy text").click()
    });
    assert_eq!(
        texts,
        [paragraph],
        "Copy text copies the stored text of the paragraph"
    );
}
