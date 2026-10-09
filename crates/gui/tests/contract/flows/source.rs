//! The Source panel shows the page its pickers and its pager name, turns to the next page with
//! no spinner in between, and shows a chapter with no page pictures as its pieces. The pickers
//! name a chapter as "Chapter N · Name" and a paper's document by its title. The Source panel or
//! the Concept Graph, once maximised, is alone in the tab until it is put back.

use std::time::{Duration, Instant};

use eframe::egui;
use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::contract::{Category, DocId, Intent, Loadable, PieceKind};
use gui::testkit;

use super::{COMMAND, Window, click, click_in, has, is_enabled, node, panels, press, says, shared};

const SOURCE_TITLE: &str = "Source in Context";
const GRAPH_TITLE: &str = "Concept Graph";
const ONE_NODE_OF_EACH_PANEL: [(Role, &str); 6] = [
    (Role::TextInput, "Question"),
    (Role::Tab, "Results"),
    (Role::Label, SOURCE_TITLE),
    (Role::Label, GRAPH_TITLE),
    (Role::Label, "Retrieval Path"),
    (Role::Label, "Follow up"),
];
/// A control in a corner of a panel is no farther than this from the corner, in points.
const CORNER_REACH: f32 = 32.0;

fn shown_page(harness: &Window) -> Option<u32> {
    shared(harness).source.page.ready().map(|view| view.page)
}

fn page_label(harness: &Window) -> String {
    let view = shared(harness)
        .source
        .page
        .ready()
        .expect("a page is shown");
    format!("page {} of {}", view.page, view.page_count)
}

fn assert_page(harness: &Window, doc: DocId, page: u32) {
    let view = shared(harness)
        .source
        .page
        .ready()
        .unwrap_or_else(|| panic!("page {page} is not ready"));
    assert_eq!((view.doc, view.page), (doc, page));
    assert!(has(harness, Role::Label, &page_label(harness)));
}

fn media_of(harness: &Window, doc: DocId) -> String {
    let catalogue = shared(harness)
        .library
        .catalogue
        .ready()
        .expect("the library loaded");
    let media = catalogue.media_of(doc).expect("the document is in a media");
    media.title.clone().expect("the media has a title")
}

fn open_the_media_list(harness: &mut Window) {
    let source = panels(DEFAULT_WINDOW).source;
    click_in(harness, Role::ComboBox, "Media", source);
}

fn shown_document(harness: &Window) -> Option<String> {
    node(harness, Role::ComboBox, "Document").value()
}

/// Queues a click and runs no frame, so that a test can look at every frame that follows.
fn queue_click(harness: &mut Window, role: Role, name: &str) {
    let at = node(harness, role, name).rect().center();
    harness.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
}

fn place(harness: &Window, role: Role, name: &str) -> Option<egui::Rect> {
    has(harness, role, name).then(|| node(harness, role, name).rect())
}

fn places_of_the_panels(harness: &Window) -> Vec<Option<egui::Rect>> {
    ONE_NODE_OF_EACH_PANEL
        .iter()
        .map(|(role, name)| place(harness, *role, name))
        .collect()
}

fn is_near(found: Option<egui::Rect>, corner: egui::Pos2) -> bool {
    found.is_some_and(|rect| rect.distance_to_pos(corner) <= CORNER_REACH)
}

/// Both panels have a button named Maximise, so the one to click is found by `area`, the place
/// of its panel.
fn maximise_makes_it_the_only_panel(harness: &mut Window, title: &str, area: egui::Rect) {
    let body = panels(DEFAULT_WINDOW).page;
    click_in(harness, Role::Button, "Maximise", area);
    for (role, name) in ONE_NODE_OF_EACH_PANEL {
        assert_eq!(
            has(harness, role, name),
            name == title,
            "`{title}` is maximised, so it alone is drawn: `{name}`"
        );
    }
    assert!(
        is_near(place(harness, Role::Label, title), body.left_top()),
        "the title of `{title}` is at the top left of the tab's body"
    );
    assert!(
        is_near(place(harness, Role::Button, "Restore"), body.right_top()),
        "Restore is at the top right of the tab's body"
    );
}

fn a_maximised_panel_is_alone_in_the_tab_until_it_is_put_back(
    harness: &mut Window,
    page_picture: &str,
) {
    let rects = panels(DEFAULT_WINDOW);
    let before = places_of_the_panels(harness);
    assert!(
        before.iter().all(Option::is_some),
        "every panel of the Ask tab is drawn"
    );
    let assert_all_are_back = |harness: &Window, way: &str| {
        let now = places_of_the_panels(harness);
        assert_eq!(now, before, "{way} puts every panel back where it was");
    };

    maximise_makes_it_the_only_panel(harness, GRAPH_TITLE, rects.concept_graph);
    click(harness, Role::Button, "Restore");
    assert_all_are_back(harness, "Restore");

    let width = |harness: &Window| node(harness, Role::Image, page_picture).rect().width();
    let narrow = width(harness);
    maximise_makes_it_the_only_panel(harness, SOURCE_TITLE, rects.source);
    assert!(width(harness) > narrow, "the page takes the larger room");
    let fit = place(harness, Role::Button, "Fit to width");
    assert!(
        is_near(fit, rects.page.right_bottom()),
        "the zoom bar is at the foot of the tab's body"
    );
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
    assert_all_are_back(harness, "Escape");

    maximise_makes_it_the_only_panel(harness, GRAPH_TITLE, rects.concept_graph);
    click(harness, Role::Tab, "Library");
    click(harness, Role::Tab, "Ask");
    assert_all_are_back(harness, "a click on a tab");

    maximise_makes_it_the_only_panel(harness, SOURCE_TITLE, rects.source);
    press(harness, COMMAND, egui::Key::K);
    assert_all_are_back(harness, "⌘K, which needs the question box,");
}

fn a_chapter_with_no_page_pictures_shows_its_pieces(harness: &mut Window, doc: DocId) {
    let notes_media = media_of(harness, doc);
    open_the_media_list(harness);
    click(harness, Role::Button, &notes_media);
    click(harness, Role::ComboBox, "Document");
    let chapter = shared(harness)
        .library
        .catalogue
        .ready()
        .and_then(|catalogue| catalogue.document(doc))
        .and_then(|document| document.chapter.clone())
        .expect("the chapter of the formula has a label");
    let row = format!("Chapter {} · {}", chapter.number, chapter.name);
    click(harness, Role::Button, &row);
    assert_eq!(shown_document(harness), Some(row));
    assert_page(harness, doc, 1);
    let view = shared(harness)
        .source
        .page
        .ready()
        .expect("the page is ready")
        .clone();
    assert!(view.image.is_none(), "this chapter has no page pictures");
    assert!(!has(harness, Role::Image, "Picture of page 1"));
    let heading = view
        .pieces
        .iter()
        .find(|piece| matches!(piece.kind, PieceKind::Heading { .. }))
        .expect("the page has a heading");
    assert!(
        has(harness, Role::Label, &heading.text),
        "the pieces of the page are shown in place of its picture"
    );
    let source = &shared(harness).source;
    assert!(
        !matches!(source.page, Loadable::Failed(_))
            && !matches!(source.concepts, Loadable::Failed(_)),
        "nothing failed"
    );
}

#[test]
fn the_source_shows_the_page_its_pickers_and_pager_name() {
    let mut harness = testkit::app("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    let figure = shared(&harness)
        .ask
        .result(8)
        .expect("the figure result")
        .clone();
    let notes = shared(&harness)
        .ask
        .result(1)
        .expect("the formula result")
        .clone();
    let with_pictures = figure.doc;

    let media = media_of(&harness, with_pictures);
    open_the_media_list(&mut harness);
    click(&mut harness, Role::Button, &media);
    assert_page(&harness, with_pictures, 1);
    assert!(has(&harness, Role::Image, "Picture of page 1"));

    click(&mut harness, Role::Button, "Next page");
    assert_page(&harness, with_pictures, 2);
    press(&mut harness, COMMAND, egui::Key::CloseBracket);
    assert_page(&harness, with_pictures, 3);
    press(&mut harness, COMMAND, egui::Key::OpenBracket);
    assert_page(&harness, with_pictures, 2);
    for _ in 0..10 {
        press(&mut harness, COMMAND, egui::Key::CloseBracket);
    }
    assert_page(&harness, with_pictures, 7);
    assert!(
        !is_enabled(&harness, Role::Button, "Next page"),
        "the last page has no next"
    );

    // The neighbour page is ready before it is asked for: the page that arrives is drawn with
    // its picture in the frame it arrives in.
    for _ in 0..3 {
        press(&mut harness, COMMAND, egui::Key::OpenBracket);
    }
    assert_page(&harness, with_pictures, 4);
    queue_click(&mut harness, Role::Button, "Next page");
    let started = Instant::now();
    loop {
        harness.step();
        if shown_page(&harness) == Some(5) {
            break;
        }
        assert!(
            has(&harness, Role::Image, "Picture of page 4"),
            "the old page stays until the next one arrives"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "page 5 never arrived"
        );
    }
    assert!(
        has(&harness, Role::Image, "Picture of page 5"),
        "the page that arrives has its picture in the same frame"
    );
    testkit::settle(&mut harness);

    harness
        .state_mut()
        .push(Intent::SelectResult(figure.number));
    testkit::settle(&mut harness);
    assert!(
        says(&harness, "on the page"),
        "the figure is framed on the page"
    );
    testkit::save_png(&mut harness, "app-source");

    let picture = format!("Picture of page {}", figure.page);
    a_maximised_panel_is_alone_in_the_tab_until_it_is_put_back(&mut harness, &picture);

    a_chapter_with_no_page_pictures_shows_its_pieces(&mut harness, notes.doc);
    a_paper_is_named_by_its_title(&mut harness);
}

/// The paper of the sample library has no folder, so its page fails to open. This step comes last
/// for that reason.
fn a_paper_is_named_by_its_title(harness: &mut Window) {
    let (title, document) = shared(harness)
        .library
        .catalogue
        .ready()
        .and_then(|catalogue| {
            let paper = catalogue
                .media
                .iter()
                .find(|media| media.category == Category::Paper)?;
            Some((paper.title.clone()?, paper.documents.first()?.title.clone()))
        })
        .expect("the library has a titled paper with a document");
    assert_ne!(
        document, title,
        "the paper's document has a title of its own, so the Document list cannot show the media's by mistake"
    );
    open_the_media_list(harness);
    click(harness, Role::Button, &title);
    assert_eq!(shown_document(harness), Some(document));
}
