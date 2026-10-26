//! The filters offer what the library holds and an ask sends what was chosen, on a wide window and
//! through the sheet of the Ask bar of a phone or a tablet, which fits its window. A follow-up
//! starts a new ask with the same mode and filters, a plain key stays in the question box, and
//! every way to copy reaches the clipboard.

use contract::{AnswerBlock, Catalogue, Category, Command, DocId, Filters};
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::{DEFAULT_WINDOW, PHONE_WINDOW, TABLET_WINDOW};
use gui::testkit;
use gui::theme::size;
use uuid::Uuid;

use super::recording::{self, Seen};
use super::{
    Window, are_stacked_in_order, click, click_in, copied_by, has, is_enabled, is_open_tab, node,
    node_in, panels, press, retype, shared,
};

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
    let paper = Paper::of(&catalogue);
    let bar = panels(DEFAULT_WINDOW).ask_bar;

    click_in(harness, Role::ComboBox, "Media", bar);
    assert!(has(harness, Role::Button, "All media"));
    for media in &with_a_document {
        assert!(
            has(harness, Role::Button, media),
            "`{media}` is a row of Media"
        );
    }
    click(harness, Role::Button, &paper.title);

    click_in(harness, Role::ComboBox, "Authors", bar);
    click(harness, Role::Button, &paper.author);

    click_in(harness, Role::ComboBox, "Tags", bar);
    click(harness, Role::CheckBox, &paper.tag);
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
    assert_eq!(
        filters_of_the_last_ask(seen),
        Some(paper.filters()),
        "the ask sends the filters that were chosen"
    );
}

/// The paper of the library, by its title, its first author and its first tag.
struct Paper {
    title: String,
    author: String,
    tag: String,
}

impl Paper {
    fn of(catalogue: &Catalogue) -> Paper {
        let paper = catalogue
            .media
            .iter()
            .find(|media| media.category == Category::Paper)
            .expect("the library has a paper");
        Paper {
            title: paper.title.clone().expect("the paper has a title"),
            author: paper
                .authors
                .first()
                .cloned()
                .expect("the paper has authors"),
            tag: paper.tags.first().cloned().expect("the paper has a tag"),
        }
    }

    fn filters(&self) -> Filters {
        Filters {
            document: None,
            media: Some(self.title.clone()),
            author: Some(self.author.clone()),
            tags: vec![self.tag.clone()],
            category: Some(Category::Paper),
        }
    }
}

fn filters_of_the_last_ask(seen: &Seen) -> Option<Filters> {
    seen.all()
        .into_iter()
        .rev()
        .find_map(|command| match command {
            Command::Ask { ask, .. } => Some(ask.filters),
            _ => None,
        })
}

const FILTER_BOXES: [&str; 5] = ["Media", "Document filter", "Authors", "Tags", "Category"];

#[test]
fn the_ask_bar_fits_a_phone_and_a_tablet_and_asks_with_the_filters_of_its_sheet() {
    let windows = [
        ("phone", PHONE_WINDOW, "Filters"),
        ("tablet", TABLET_WINDOW, "More filters"),
    ];
    for (name, window, more) in windows {
        let (mut harness, seen) = recording::open("black-scholes", window);
        testkit::settle(&mut harness);
        let bar = panels(window).ask_bar;
        let library = &shared(&harness).library;
        let paper = Paper::of(library.catalogue.ready().expect("the library loaded"));

        let names = assert_the_bar_fits(&harness, window);
        let mut on_the_bar = vec!["Question", "Mode", "Ask", more];
        if window == TABLET_WINDOW {
            on_the_bar.extend(["Media", "Document filter"]);
        }
        for control in on_the_bar.iter().chain(&FILTER_BOXES) {
            let is_there = names.iter().any(|name| name == control);
            let is_wanted = on_the_bar.contains(control);
            assert_eq!(is_there, is_wanted, "{name}: `{control}` on the bar");
        }
        testkit::save_png(&mut harness, &format!("{name}-ask-bar"));

        if window == PHONE_WINDOW {
            let question = shared(&harness).ask.question.clone();
            click(&mut harness, Role::TextInput, "Question");
            harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
            harness.step();
            harness.key_press(egui::Key::Backspace);
            testkit::settle(&mut harness);
            assert_eq!(question_box(&harness).as_deref(), Some(""));
            let names = assert_the_bar_fits(&harness, window);
            assert!(
                names.iter().any(|name| name == "Type a question to ask."),
                "the note shows"
            );
            retype(&mut harness, "Question", &question);
        }

        // A filter on the bar is chosen where it stands. The rest are chosen in the sheet.
        let mut in_the_sheet = Vec::new();
        for filter in ["Media", "Authors", "Tags", "Category"] {
            if node_in(&harness, Role::ComboBox, filter, bar).is_some() {
                choose(&mut harness, filter, bar, &paper);
            } else {
                in_the_sheet.push(filter);
            }
        }
        click_in(&mut harness, Role::Button, more, bar);
        let heading = node(&harness, Role::Label, more).rect();
        let sheet = heading.union(node(&harness, Role::Button, "Done").rect());
        for filter in FILTER_BOXES {
            let places = harness
                .query_all_by_role_and_label(Role::ComboBox, filter)
                .filter(|found| {
                    [bar, sheet]
                        .iter()
                        .any(|area| area.contains(found.rect().center()))
                })
                .count();
            assert_eq!(
                places, 1,
                "{name}: `{filter}` stands once, on the bar or in the sheet"
            );
        }
        testkit::save_png(&mut harness, &format!("{name}-filters-sheet"));
        for filter in &in_the_sheet {
            choose(&mut harness, filter, sheet, &paper);
        }
        let count = node_in(&harness, Role::Button, more, bar).and_then(|button| button.value());
        let set_in_the_sheet = in_the_sheet.len().to_string();
        assert_eq!(
            count.as_deref(),
            Some(set_in_the_sheet.as_str()),
            "{name}: filters set in the sheet"
        );
        click(&mut harness, Role::Button, "Done");
        let modal = harness.ctx.memory(|memory| memory.top_modal_layer());
        assert!(modal.is_none(), "Done closes the sheet");

        assert_the_bar_fits(&harness, window);
        click_in(&mut harness, Role::Button, "Ask", bar);
        assert_eq!(
            filters_of_the_last_ask(&seen),
            Some(paper.filters()),
            "{name}: what the ask sends"
        );
    }
}

/// Every control of the bar lies in the bar and in the window, and none covers another. On a
/// phone each one is tall enough for a finger. Returns their names.
fn assert_the_bar_fits(harness: &Window, window: [f32; 2]) -> Vec<String> {
    let bar = panels(window).ask_bar.expand(0.5);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::from(window)).expand(0.5);
    let touched = [Role::TextInput, Role::ComboBox, Role::Button];
    let roles = touched.into_iter().chain([Role::CheckBox, Role::Label]);
    let controls: Vec<(String, Role, egui::Rect)> = roles
        .flat_map(|role| {
            harness
                .query_all_by_role(role)
                .map(move |node| (role, node))
        })
        .filter(|(_, control)| bar.contains(control.rect().center()))
        .map(|(role, control)| {
            // A label keeps its words as its value.
            let name = control.accesskit_node().label().or_else(|| control.value());
            let name = name.unwrap_or_default();
            (name, role, control.rect())
        })
        .collect();
    for (at, (name, role, rect)) in controls.iter().enumerate() {
        assert!(
            bar.contains_rect(*rect) && screen.contains_rect(*rect),
            "`{name}` {rect:?} leaves the bar {bar:?} or the window"
        );
        let is_finger_sized = rect.height() >= size::CONTROL_MD;
        assert!(
            window != PHONE_WINDOW || !touched.contains(role) || is_finger_sized,
            "`{name}` is {} tall",
            rect.height()
        );
        for (other, _, other_rect) in &controls[at + 1..] {
            let both = rect.intersect(*other_rect);
            assert!(
                both.width() <= 0.0 || both.height() <= 0.0,
                "`{name}` {rect:?} covers `{other}` {other_rect:?}"
            );
        }
    }
    controls.into_iter().map(|(name, ..)| name).collect()
}

/// Opens the list `filter` in `area` and picks what `paper` stands for.
fn choose(harness: &mut Window, filter: &str, area: egui::Rect, paper: &Paper) {
    click_in(harness, Role::ComboBox, filter, area);
    match filter {
        "Media" => click(harness, Role::Button, &paper.title),
        "Authors" => click(harness, Role::Button, &paper.author),
        "Tags" => {
            click(harness, Role::CheckBox, &paper.tag);
            press(harness, egui::Modifiers::NONE, egui::Key::Escape);
        }
        "Category" => click(harness, Role::Button, "Paper"),
        other => panic!("`{other}` is not a filter to choose"),
    }
}

#[test]
fn the_document_filter_offers_the_documents_of_the_chosen_media_and_an_ask_searches_one() {
    let (mut harness, seen) = recording::open("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    let bar = panels(DEFAULT_WINDOW).ask_bar;
    assert!(
        !is_enabled(&harness, Role::ComboBox, "Document filter"),
        "no media is chosen, so there is no list of documents to offer"
    );

    click_in(&mut harness, Role::ComboBox, "Media", bar);
    click(&mut harness, Role::Button, "Quanty Sample Notes");
    click_in(&mut harness, Role::ComboBox, "Document filter", bar);
    let rows = [
        "All documents",
        "Chapter 1 · Options Pricing Intuition",
        "Chapter 2 · Black Scholes In Depth",
        "Index",
    ];
    for row in rows {
        assert!(has(&harness, Role::Button, row), "`{row}` is a row");
    }
    assert!(
        are_stacked_in_order(&harness, Role::Button, &rows),
        "chapters by number, then the other documents"
    );
    click(&mut harness, Role::Button, "Index");
    click(&mut harness, Role::Button, "Ask");

    assert_eq!(
        filters_of_the_last_ask(&seen),
        Some(Filters {
            media: Some("Quanty Sample Notes".to_owned()),
            document: Some(DocId(Uuid::from_u128(5))),
            ..Filters::default()
        })
    );
    let searched = shared(&harness)
        .ask
        .search
        .ready()
        .map(|reply| reply.trace.documents_searched);
    assert_eq!(searched, Some(Some(1)), "the index alone was searched");

    click_in(&mut harness, Role::ComboBox, "Media", bar);
    click(&mut harness, Role::Button, "Option Volatility and Pricing");
    click(&mut harness, Role::Button, "Ask");
    assert_eq!(
        filters_of_the_last_ask(&seen).map(|filters| filters.document),
        Some(None),
        "another media clears the document"
    );
}

fn copies_reach_the_clipboard(harness: &mut Window) {
    let markdown = shared(harness)
        .ask
        .answer_markdown()
        .expect("an answer to share");
    let texts = copied_by(harness, |harness| {
        click(harness, Role::Button, "Share");
        node(harness, Role::Button, "Copy as Markdown").click();
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
    let title = shared(harness)
        .ask
        .answer
        .ready()
        .and_then(|answer| answer.title.clone())
        .expect("the answer has a title");
    assert!(
        texts[0].starts_with(&format!("# {title}\n")),
        "⇧⌘S copies the answer with its title as the first line"
    );

    let first_formula = shared(harness)
        .ask
        .results()
        .iter()
        .find(|result| result.kind == contract::ItemKind::Formula)
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
