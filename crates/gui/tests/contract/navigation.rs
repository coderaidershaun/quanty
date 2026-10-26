//! Checks the Ask views and Back: the rules on the state, and the phone and tablet journeys.

use contract::{
    AskDraft, AskView, Catalogue, Command, DocId, Document, Effect, Event, Intent, Media, Panel,
    Tab,
};
use eframe::egui::accesskit::Role;
use eframe::egui::{Pos2, Rect, Vec2};
use egui_kittest::kittest::NodeT as _;
use gui::app::layout::{PHONE_WINDOW, ShellRects, TABLET_WINDOW};
use gui::state::{Open, Shared};
use gui::testkit::{self, sample};
use uuid::Uuid;

use crate::flows::{Wheel, Window, click_in, has, node_in, panels, scroll_to, shared as state_of};

const MEDIA: &str = "Dynamic Hedging";

fn answered() -> Shared {
    testkit::answered(
        sample::search_reply(),
        sample::concept_graph(),
        sample::answer(),
    )
}

/// Applies the intent, then checks that `can_go_back` still says whether Back would change
/// anything.
fn apply(shared: &mut Shared, intent: Intent) {
    shared.apply_intent(intent, &mut Vec::new());
    let mut after_back = shared.clone();
    after_back.apply_intent(Intent::Back, &mut Vec::new());
    assert_eq!(
        shared.can_go_back(),
        after_back != *shared,
        "can_go_back must be true exactly when Back changes something (tab {:?}, maximised {:?}, view {:?})",
        shared.tab,
        shared.maximised,
        shared.ask_view
    );
}

#[test]
fn the_ask_views_and_back_follow_their_rules() {
    let mut shared = answered();
    assert_eq!(shared.ask_view, AskView::Answer);
    assert_eq!(
        shared.side_view(),
        AskView::Source,
        "Source until one is chosen"
    );
    assert!(
        !shared.can_go_back(),
        "a fresh answer has nothing to go back to"
    );

    apply(&mut shared, Intent::ShowAskView(AskView::ConceptGraph));
    assert_eq!(shared.ask_view, AskView::ConceptGraph);
    assert_eq!(shared.side_view(), AskView::ConceptGraph);

    apply(&mut shared, Intent::ShowAskView(AskView::Answer));
    assert_eq!(shared.ask_view, AskView::Answer);
    assert_eq!(
        shared.side_view(),
        AskView::ConceptGraph,
        "the Answer never becomes the side view"
    );

    apply(&mut shared, Intent::OpenTab(Tab::Library));
    apply(&mut shared, Intent::ShowAskView(AskView::RetrievalPath));
    assert_eq!(shared.tab, Tab::Ask, "it opens the Ask tab");
    apply(&mut shared, Intent::Maximise(Panel::Source));
    apply(&mut shared, Intent::ShowAskView(AskView::FollowUp));
    assert_eq!(shared.maximised, None, "it puts a maximised panel back");

    new_asks_and_focus_return_to_the_answer(&mut shared);
    choosing_a_result_shows_the_source();
    back_undoes_one_step_at_a_time();
}

fn new_asks_and_focus_return_to_the_answer(shared: &mut Shared) {
    let ask = |question: &str| {
        Intent::Ask(AskDraft {
            question: question.to_owned(),
            ..AskDraft::default()
        })
    };

    apply(shared, Intent::ShowAskView(AskView::ConceptGraph));
    apply(shared, ask("   "));
    assert_eq!(
        shared.ask_view,
        AskView::ConceptGraph,
        "a blank question asks nothing, so the view stays"
    );
    apply(shared, ask("What does gamma measure?"));
    assert_eq!(
        shared.ask_view,
        AskView::Answer,
        "a new ask shows its answer"
    );
    assert_eq!(shared.side_view(), AskView::ConceptGraph);

    apply(shared, Intent::ShowAskView(AskView::RetrievalPath));
    apply(shared, Intent::FocusAskBar);
    assert_eq!(
        shared.ask_view,
        AskView::Answer,
        "the Ask bar is drawn over the Answer only"
    );
}

fn choosing_a_result_shows_the_source() {
    let mut shared = answered();
    apply(&mut shared, Intent::ShowAskView(AskView::ConceptGraph));
    apply(&mut shared, Intent::SelectResult(2));
    assert_eq!(shared.ask_view, AskView::Source);
    assert_eq!(shared.side_view(), AskView::Source);
    assert_eq!(shared.ask.selected_result, Some(2));

    apply(&mut shared, Intent::ShowAskView(AskView::Answer));
    let before = shared.clone();
    apply(&mut shared, Intent::SelectResult(99));
    assert_eq!(
        shared, before,
        "a result that does not exist changes nothing"
    );

    let doc = shared.ask.results()[0].doc;
    apply(&mut shared, Intent::OpenTab(Tab::Library));
    apply(
        &mut shared,
        Intent::OpenSource {
            doc,
            page: 3,
            piece: None,
        },
    );
    assert_eq!((shared.tab, shared.ask_view), (Tab::Ask, AskView::Source));

    apply(&mut shared, Intent::ShowAskView(AskView::ConceptGraph));
    apply(&mut shared, Intent::StepResult(1));
    assert_eq!(
        shared.ask_view,
        AskView::ConceptGraph,
        "stepping keeps the view"
    );
    assert_eq!(
        shared.ask.selected_result,
        Some(3),
        "it went on from result 2"
    );
}

/// The state of an answered question, with a catalogue of one media that holds one document.
fn answered_with_a_library() -> (Shared, DocId) {
    let doc = DocId(Uuid::from_u128(1));
    let catalogue = Catalogue {
        media: vec![Media {
            title: Some(MEDIA.to_owned()),
            documents: vec![Document {
                id: doc,
                ..Document::default()
            }],
            ..Media::default()
        }],
    };
    let mut shared = answered();
    let mut effects = Vec::new();
    shared.apply_intent(Intent::RefreshCatalogue, &mut effects);
    let request = effects
        .iter()
        .find_map(|effect| match effect {
            Effect::Send(Command::LoadCatalogue { request }) => Some(*request),
            _ => None,
        })
        .expect("a refresh asks the backend for the catalogue");
    shared.apply_event(
        Event::Catalogue {
            request,
            result: Ok(catalogue),
        },
        &mut effects,
    );
    (shared, doc)
}

fn back_undoes_one_step_at_a_time() {
    let mut shared = answered();
    apply(&mut shared, Intent::ShowAskView(AskView::ConceptGraph));
    apply(&mut shared, Intent::Maximise(Panel::ConceptGraph));
    apply(&mut shared, Intent::Back);
    assert_eq!(
        shared.maximised, None,
        "a maximised panel is put back first"
    );
    assert_eq!(shared.ask_view, AskView::ConceptGraph);
    apply(&mut shared, Intent::Back);
    assert_eq!(shared.ask_view, AskView::Answer);
    assert_eq!(
        shared.side_view(),
        AskView::ConceptGraph,
        "the side view is kept"
    );
    let before = shared.clone();
    apply(&mut shared, Intent::Back);
    assert_eq!(
        shared, before,
        "the Answer of the Ask tab has nowhere to go back to"
    );

    apply(&mut shared, Intent::OpenTab(Tab::Ingest));
    apply(&mut shared, Intent::Back);
    assert_eq!(shared.tab, Tab::Ask);

    let (mut shared, doc) = answered_with_a_library();
    apply(&mut shared, Intent::OpenDocument(doc));
    assert_eq!(shared.shelf.open, Some(Open::Document(doc)));
    apply(&mut shared, Intent::Back);
    let media = Open::Media {
        title: Some(MEDIA.to_owned()),
    };
    assert_eq!(
        shared.shelf.open,
        Some(media),
        "a document goes back to its media"
    );
    apply(&mut shared, Intent::Back);
    assert_eq!((shared.tab, &shared.shelf.open), (Tab::Library, &None));
    apply(&mut shared, Intent::Back);
    assert_eq!(
        shared.tab,
        Tab::Ask,
        "a Library with nothing open goes back to Ask"
    );

    // A panel that was maximised stays so while the Library is open, and the Library's own
    // steps come first.
    apply(&mut shared, Intent::Maximise(Panel::Source));
    apply(&mut shared, Intent::OpenDocument(doc));
    assert_eq!(shared.maximised, Some(Panel::Source));
    apply(&mut shared, Intent::Back);
    assert_eq!(shared.maximised, Some(Panel::Source));
    assert_eq!(shared.tab, Tab::Library);
    apply(&mut shared, Intent::Back);
    apply(&mut shared, Intent::Back);
    assert_eq!((shared.tab, shared.maximised), (Tab::Ask, None));

    let mut shared = answered();
    apply(&mut shared, Intent::OpenDocument(doc));
    apply(&mut shared, Intent::Back);
    assert_eq!(
        shared.shelf.open, None,
        "a document that no catalogue lists has no media to go back to"
    );
}

/// What each view is called on the bottom navigation and on the strip over the side panel.
const VIEW_LABELS: [(AskView, &str); 5] = [
    (AskView::Answer, "Answer"),
    (AskView::Source, "Source"),
    (AskView::ConceptGraph, "Graph"),
    (AskView::RetrievalPath, "Path"),
    (AskView::FollowUp, "Follow up"),
];

fn label_of(view: AskView) -> &'static str {
    VIEW_LABELS
        .iter()
        .find_map(|(shown, label)| (*shown == view).then_some(*label))
        .expect("every view has a label")
}

fn rect_of(rects: &ShellRects, view: AskView) -> Rect {
    match view {
        AskView::Answer => rects.answer,
        AskView::Source => rects.source,
        AskView::ConceptGraph => rects.concept_graph,
        AskView::RetrievalPath => rects.retrieval_path,
        AskView::FollowUp => rects.follow_up,
    }
}

/// What a view draws that no other view draws: the Answer its `Results` tab, the others their
/// titles.
fn marker(view: AskView) -> (Role, &'static str) {
    match view {
        AskView::Answer => (Role::Tab, "Results"),
        AskView::Source => (Role::Label, "Source in Context"),
        AskView::ConceptGraph => (Role::Label, "Concept Graph"),
        AskView::RetrievalPath => (Role::Label, "Retrieval Path"),
        AskView::FollowUp => (Role::Label, "Follow up"),
    }
}

fn is_drawn_in(harness: &Window, view: AskView, area: Rect) -> bool {
    let (role, name) = marker(view);
    node_in(harness, role, name, area).is_some()
}

fn is_drawn(harness: &Window, view: AskView) -> bool {
    let (role, name) = marker(view);
    has(harness, role, name)
}

fn is_selected(harness: &Window, label: &str, area: Rect) -> bool {
    node_in(harness, Role::Tab, label, area)
        .unwrap_or_else(|| panic!("no tab named `{label}` is in {area:?}"))
        .accesskit_node()
        .is_selected()
        == Some(true)
}

fn assert_shows_only(harness: &Window, rects: &ShellRects, view: AskView) {
    assert_eq!(state_of(harness).ask_view, view);
    for other in AskView::ALL {
        if other == view {
            assert!(
                is_drawn_in(harness, view, rect_of(rects, view)),
                "the {view:?} view is drawn in its room"
            );
        } else {
            assert!(!is_drawn(harness, other), "the {other:?} view is not drawn");
        }
    }
}

fn assert_items_fit_the_bar(harness: &Window, bar: Rect) {
    let window = Rect::from_min_size(Pos2::ZERO, Vec2::from(PHONE_WINDOW));
    let mut items = Vec::new();
    for (_, label) in VIEW_LABELS {
        let item = node_in(harness, Role::Tab, label, bar)
            .unwrap_or_else(|| panic!("the bottom navigation has no item `{label}`"))
            .rect();
        assert!(
            bar.contains_rect(item) && window.contains_rect(item),
            "`{label}` {item:?}"
        );
        assert!(
            item.height() >= 44.0,
            "`{label}` is only {} tall",
            item.height()
        );
        items.push(item);
    }
    for (index, item) in items.iter().enumerate() {
        for other in &items[index + 1..] {
            let overlap = item.intersect(*other);
            assert!(
                overlap.width() <= 0.0 || overlap.height() <= 0.0,
                "{item:?} overlaps {other:?}"
            );
        }
    }
}

#[test]
fn a_phone_shows_one_view_of_the_ask_tab_at_a_time() {
    let mut harness = testkit::app("black-scholes", PHONE_WINDOW);
    testkit::settle(&mut harness);
    let rects = panels(PHONE_WINDOW);
    let bar = rects.bottom_nav.expect("a phone has a bottom navigation");
    assert_items_fit_the_bar(&harness, bar);
    assert_shows_only(&harness, &rects, AskView::Answer);
    testkit::save_png(&mut harness, "phone-answer");

    for (view, label) in VIEW_LABELS
        .into_iter()
        .cycle()
        .skip(1)
        .take(VIEW_LABELS.len())
    {
        click_in(&mut harness, Role::Tab, label, bar);
        assert_shows_only(&harness, &rects, view);
        assert!(is_selected(&harness, label, bar), "`{label}` is marked");
        if view == AskView::Source {
            testkit::save_png(&mut harness, "phone-source");
        }
        if view == AskView::FollowUp {
            testkit::save_png(&mut harness, "phone-follow-up");
        }
    }

    scroll_to(
        &mut harness,
        rects.answer,
        Wheel::Down,
        Role::Button,
        "Citation 1",
    );
    let citation = node_in(&harness, Role::Button, "Citation 1", rects.answer)
        .expect("a citation chip is in the Answer view");
    citation.click();
    testkit::settle(&mut harness);
    assert_shows_only(&harness, &rects, AskView::Source);
    assert_eq!(state_of(&harness).ask.selected_result, Some(1));

    click_in(&mut harness, Role::Tab, label_of(AskView::FollowUp), bar);
    let question = state_of(&harness)
        .ask
        .answer
        .ready()
        .and_then(|answer| answer.follow_ups.first().cloned())
        .expect("the answer suggests a follow-up");
    let generation = state_of(&harness).ask.generation;
    click_in(&mut harness, Role::Button, &question, rects.follow_up);
    assert_eq!(state_of(&harness).ask.generation, generation + 1);
    assert_shows_only(&harness, &rects, AskView::Answer);

    click_in(&mut harness, Role::Tab, label_of(AskView::Source), bar);
    harness.state_mut().push(Intent::Back);
    testkit::settle(&mut harness);
    assert_shows_only(&harness, &rects, AskView::Answer);
    assert!(!state_of(&harness).can_go_back());
}

#[test]
fn a_tablet_shows_the_answer_beside_the_chosen_side_panel() {
    let mut harness = testkit::app("black-scholes", TABLET_WINDOW);
    testkit::settle(&mut harness);
    let rects = panels(TABLET_WINDOW);
    let tabs = rects
        .side_tabs
        .expect("a tablet has a strip over the side panel");
    let both = |harness: &Window, side: AskView| {
        assert!(
            is_drawn_in(harness, AskView::Answer, rects.answer),
            "the Answer stays on screen"
        );
        assert!(
            is_drawn_in(harness, side, rect_of(&rects, side)),
            "the {side:?} view is in the side panel"
        );
        for other in AskView::ALL {
            if other != AskView::Answer && other != side {
                assert!(!is_drawn(harness, other), "the {other:?} view is not drawn");
            }
        }
    };
    both(&harness, AskView::Source);
    for (_, label) in &VIEW_LABELS[1..] {
        let tab = node_in(&harness, Role::Tab, label, tabs)
            .unwrap_or_else(|| panic!("the strip has no tab `{label}`"))
            .rect();
        assert!(tabs.contains_rect(tab), "`{label}` {tab:?}");
    }
    testkit::save_png(&mut harness, "tablet-source");

    click_in(
        &mut harness,
        Role::Tab,
        label_of(AskView::ConceptGraph),
        tabs,
    );
    both(&harness, AskView::ConceptGraph);
    assert_eq!(state_of(&harness).side_view(), AskView::ConceptGraph);

    scroll_to(
        &mut harness,
        rects.answer,
        Wheel::Down,
        Role::Button,
        "Citation 1",
    );
    let citation = node_in(&harness, Role::Button, "Citation 1", rects.answer)
        .expect("a citation chip is in the Answer");
    citation.click();
    testkit::settle(&mut harness);
    both(&harness, AskView::Source);
    assert!(is_selected(&harness, label_of(AskView::Source), tabs));
}
