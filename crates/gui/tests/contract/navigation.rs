//! Checks the Ask views and Back: the rules on the state.

use contract::{
    AskDraft, AskView, Catalogue, Command, DocId, Document, DocumentView, Effect, Event, Intent,
    Media, Panel, Tab,
};
use gui::state::{Open, Shared};
use gui::testkit::{self, sample};
use uuid::Uuid;

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

    // A Source view that "Read" opened from the Library goes back to the Library, on the page it
    // left, in every window.
    let (mut shared, doc) = answered_with_a_library();
    apply(&mut shared, Intent::OpenDocument(doc));
    apply(
        &mut shared,
        Intent::ShowDocumentView(DocumentView::Insights),
    );
    let library_page = shared.shelf.clone();
    let read = Intent::OpenSource {
        doc,
        page: 3,
        piece: None,
    };
    apply(&mut shared, read.clone());
    assert_eq!((shared.tab, shared.ask_view), (Tab::Ask, AskView::Source));
    apply(&mut shared, Intent::Back);
    assert_eq!(shared.tab, Tab::Library);
    assert_eq!(
        shared.shelf, library_page,
        "the Library shows the same page it had"
    );

    // Choosing another page inside that Source view is still the Library's Source view.
    apply(&mut shared, read.clone());
    apply(&mut shared, read.clone());
    apply(&mut shared, Intent::Back);
    assert_eq!(shared.tab, Tab::Library);

    // A view chosen by hand, or a result stepped to, is not where "Read" took the person, so Back
    // goes to the Answer as it does for any Source view.
    apply(&mut shared, read.clone());
    apply(&mut shared, Intent::ShowAskView(AskView::Answer));
    apply(&mut shared, Intent::ShowAskView(AskView::Source));
    apply(&mut shared, Intent::Back);
    assert_eq!((shared.tab, shared.ask_view), (Tab::Ask, AskView::Answer));

    apply(&mut shared, Intent::OpenTab(Tab::Library));
    apply(&mut shared, read);
    apply(&mut shared, Intent::StepResult(1));
    assert_eq!(shared.ask_view, AskView::Source);
    apply(&mut shared, Intent::Back);
    assert_eq!((shared.tab, shared.ask_view), (Tab::Ask, AskView::Answer));
}
