//! The filters offer what the library holds and an ask sends what was chosen, a follow-up starts a
//! new ask with the same mode and filters, a plain key stays in the question box, and every way to
//! copy reaches the clipboard.

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::contract::{AnswerBlock, Category, Command, Filters};
use gui::testkit;

use super::recording::{self, Seen};
use super::{Window, click, click_in, copied_by, has, is_open_tab, node, panels, press, shared};

fn question_box(harness: &Window) -> Option<String> {
    node(harness, Role::TextInput, "Question").value()
}

fn assert_new_ask(harness: &Window, generation: u64, question: &str) {
    let ask = &shared(harness).ask;
    assert_eq!(ask.generation, generation + 1, "one new ask was made");
    assert_eq!(ask.question, question);
}

#[test]
fn a_follow_up_starts_a_new_ask_and_copies_reach_the_clipboard() {
    let (mut harness, seen) = recording::open("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);

    let before = shared(&harness).ask.generation;
    let question = shared(&harness).ask.question.clone();
    assert_eq!(
        question_box(&harness).as_deref(),
        Some(question.as_str()),
        "the bar holds the question of the ask on screen"
    );
    the_filters_offer_the_library_and_an_ask_sends_them(&mut harness, &seen);
    assert_new_ask(&harness, before, &question);

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

/// The four filters together fit only the paper of the library, so the ask still has an answer
/// for the follow-up to start from.
fn the_filters_offer_the_library_and_an_ask_sends_them(harness: &mut Window, seen: &Seen) {
    let catalogue = shared(harness)
        .library
        .catalogue
        .ready()
        .expect("the library loaded")
        .clone();
    let with_a_document: Vec<&str> = catalogue
        .media
        .iter()
        .filter(|media| !media.documents.is_empty())
        .filter_map(|media| media.title.as_deref())
        .collect();
    let paper = catalogue
        .media
        .iter()
        .find(|media| media.category == Category::Paper)
        .expect("the library has a paper");
    let title = paper.title.clone().expect("the paper has a title");
    let author = paper
        .authors
        .first()
        .expect("the paper has authors")
        .clone();
    let media_tag = paper.tags.first().expect("the paper has a tag").clone();
    let bar = panels(DEFAULT_WINDOW).ask_bar;

    click_in(harness, Role::ComboBox, "Media", bar);
    assert!(has(harness, Role::Button, "All media"));
    for media in &with_a_document {
        assert!(
            has(harness, Role::Button, media),
            "`{media}` is a row of Media"
        );
    }
    click(harness, Role::Button, &title);

    click_in(harness, Role::ComboBox, "Authors", bar);
    click(harness, Role::Button, &author);

    click_in(harness, Role::ComboBox, "Tags", bar);
    click(harness, Role::CheckBox, &media_tag);
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);

    click_in(harness, Role::ComboBox, "Category", bar);
    for row in ["Any category", "Book", "Paper", "Other"] {
        assert!(
            has(harness, Role::Button, row),
            "`{row}` is a row of Category"
        );
    }
    click(harness, Role::Button, "Paper");

    click(harness, Role::Button, "Ask");
    let sent = seen
        .all()
        .into_iter()
        .rev()
        .find_map(|command| match command {
            Command::Ask { ask, .. } => Some(ask.filters),
            _ => None,
        });
    assert_eq!(
        sent,
        Some(Filters {
            media: Some(title),
            author: Some(author),
            tags: vec![media_tag],
            category: Some(Category::Paper),
        }),
        "the ask sends the filters that were chosen"
    );
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
