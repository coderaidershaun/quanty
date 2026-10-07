//! What the panel says when there is no page to show: nothing open, opening, or not opened.

use eframe::egui;

use crate::contract::{Failure, FailureKind, Intent, Loadable};
use crate::panels::PanelCx;
use crate::theme::Icon;
use crate::widgets::{Button, ControlSize, Placeholder, PlaceholderResponse};

/// In a room lower than this a placeholder sits in a scroll area, so none of it is cut off.
const MIN_ROOM: f32 = 200.0;

const RETRY: &str = "Try again";

pub(super) fn show(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    match &cx.shared.source.page {
        Loadable::Loading => {
            whole_area(ui, Placeholder::loading("Opening the page"));
        }
        Loadable::Failed(failure) => failed(ui, failure, cx),
        Loadable::Idle | Loadable::Ready(_) => nothing_open(ui, cx),
    }
}

fn nothing_open(ui: &mut egui::Ui, cx: &PanelCx<'_>) {
    let hint = match cx.shared.library.catalogue.ready() {
        Some(catalogue) if catalogue.documents().next().is_none() => {
            "The library is empty. Add a chapter on the Ingest tab to read it here."
        }
        Some(_) => "Select a result, or choose a book above, to see the page it stands on.",
        None => "Select a result to see the page it stands on.",
    };
    whole_area(
        ui,
        Placeholder::empty(Icon::DOCUMENT, "No page open").hint(hint),
    );
}

fn failed(ui: &mut egui::Ui, failure: &Failure, cx: &mut PanelCx<'_>) {
    let title = match failure.kind {
        FailureKind::SourceMissing => "The page was not found",
        _ => "The page could not be opened",
    };
    let placeholder = Placeholder::error(title).hint(&failure.hint);
    let is_retry_clicked = if ui.available_height() >= MIN_ROOM {
        let shown = whole_area(ui, placeholder.action(RETRY));
        shown.response.on_hover_text(&failure.detail);
        shown.action_clicked
    } else {
        retry_below_scrolling_text(ui, placeholder, &failure.detail)
    };
    if let (true, Some(target)) = (is_retry_clicked, cx.shared.source.target) {
        cx.intents.push(Intent::OpenSource {
            doc: target.doc,
            page: target.page,
            piece: target.piece,
        });
    }
}

/// In a small room the button is fixed at the foot, so it is in view whatever the length of the
/// hint, and only the text above it scrolls.
fn retry_below_scrolling_text(
    ui: &mut egui::Ui,
    placeholder: Placeholder<'_>,
    detail: &str,
) -> bool {
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
        let retry = Button::secondary(RETRY).size(ControlSize::Small);
        let is_clicked = ui.add(retry).clicked();
        whole_area(ui, placeholder).response.on_hover_text(detail);
        is_clicked
    })
    .inner
}

pub(super) fn whole_area(ui: &mut egui::Ui, placeholder: Placeholder<'_>) -> PlaceholderResponse {
    if ui.available_height() >= MIN_ROOM {
        return placeholder.show(ui);
    }
    egui::ScrollArea::vertical()
        .show(ui, |ui| placeholder.show(ui))
        .inner
}

#[cfg(test)]
mod tests {
    use eframe::egui::Rect;
    use eframe::egui::accesskit::Role;
    use egui_kittest::Harness;
    use egui_kittest::kittest::{By, NodeT, Queryable};

    use crate::contract::{Catalogue, Event, Failure, FailureKind, ImageRef, Intent, PieceKind};
    use crate::panels::source::page::PICTURE_ALT;
    use crate::panels::source::samples::{
        self, DEFAULT_SIZE as SIZE, SMALLEST_SIZE as SMALLEST, Sample, has_picture_of,
        shows as says,
    };
    use crate::state::Shared;
    use crate::testkit::{self, Host, sample};

    /// A hint with a long folder path in it, as the real one has. It wraps to many lines, so the
    /// placeholder under it needs more room than the smallest panel gives.
    const LONG_HINT: &str = "quanty does not know where this chapter's pages are. Put its chapter folder inside a book folder under /Users/someone/Code/quanty/crates/gui/tests/fixtures/samples, or ingest its PDF again with rag-ingest pdf.";

    /// True when a spinner that is named for this wait is on screen. A spinner of something else,
    /// such as a formula that is being typeset, does not count.
    fn is_spinning_for(harness: &Harness<'static, Host>, wait: &str) -> bool {
        let by = By::new().role(Role::ProgressIndicator).label_contains(wait);
        harness.query_all(by).next().is_some()
    }

    /// The state after the page of `Sample::FigureTop` was asked for and the backend failed.
    fn failed(failure: Failure) -> Shared {
        let mut shared = samples::library(samples::catalogue());
        shared.apply_intent(open(), &mut Vec::new());
        let request = shared.source.pending.expect("the page is loading");
        let page = Event::Page {
            request,
            result: Err(failure.clone()),
        };
        shared.apply_event(page, &mut Vec::new());
        let concepts = Event::PageConcepts {
            request,
            result: Err(failure),
        };
        shared.apply_event(concepts, &mut Vec::new());
        shared
    }

    fn open() -> Intent {
        Intent::OpenSource {
            doc: Sample::FigureTop.doc(),
            page: Sample::FigureTop.page_number(),
            piece: None,
        }
    }

    /// A failed page with a long hint in the smallest panel: the button under the hint is in view
    /// when the panel opens, and it opens the page again.
    fn a_long_hint_leaves_the_button_in_view() {
        let mut failure = sample::failure(FailureKind::SourceMissing);
        failure.hint = LONG_HINT.to_owned();
        let mut harness = samples::panel(SMALLEST, failed(failure));
        harness.run();
        let again = harness.get_by_label("Try again").rect();
        let room = samples::panel_room(SMALLEST);
        assert!(room.contains_rect(again), "{again:?} in {room:?}");
        let named = harness.query_all_by_label_contains("The page was not found");
        let title = named.map(|node| node.rect()).reduce(Rect::union);
        let title = title.expect("the placeholder is drawn");
        assert!(
            room.top() <= title.top(),
            "the text starts in view: {title:?}"
        );
        testkit::save_png(&mut harness, "source-missing-small");
        harness.get_by_label("Try again").click();
        harness.run();
        assert_eq!(harness.state().intents, vec![open()]);
    }

    /// After a failed page the pager has nothing to turn: both buttons are off and a click on
    /// either pushes nothing.
    fn the_pager_of_a_failed_page_is_off(harness: &mut Harness<'static, Host>) {
        for label in ["Previous page", "Next page"] {
            assert!(harness.get_by_label(label).accesskit_node().is_disabled());
            harness.get_by_label(label).click();
            harness.run();
        }
        assert!(harness.state().intents.is_empty());
    }

    #[test]
    fn every_state_without_a_page_says_what_to_do() {
        let mut harness = samples::panel(SIZE, samples::library(Catalogue::default()));
        harness.run();
        assert!(says(&harness, "No page open"));
        assert!(says(
            &harness,
            "The library is empty. Add a chapter on the Ingest tab to read it here."
        ));
        assert!(!says(&harness, "Open Ingest"), "the button is not built");
        testkit::save_png(&mut harness, "source-empty");

        harness.state_mut().shared = samples::library(samples::catalogue());
        harness.run();
        assert!(says(
            &harness,
            "Select a result, or choose a book above, to see the page it stands on."
        ));

        harness.state_mut().shared = Shared::default();
        harness.run();
        assert!(says(
            &harness,
            "Select a result to see the page it stands on."
        ));

        let mut shared = samples::library(samples::catalogue());
        shared.apply_intent(open(), &mut Vec::new());
        harness.state_mut().shared = shared;
        harness.run();
        assert!(says(&harness, "Opening the page"));
        assert!(is_spinning_for(&harness, "Opening the page"));
        testkit::save_png(&mut harness, "source-opening");

        harness.state_mut().shared = failed(sample::failure(FailureKind::SourceMissing));
        harness.run();
        assert!(says(&harness, "The page was not found"));
        assert!(says(&harness, FailureKind::SourceMissing.hint()));
        assert!(
            !says(&harness, "Opening the page"),
            "a failure is not a wait"
        );
        testkit::save_png(&mut harness, "source-missing");
        harness.get_by_label("Try again").click();
        harness.run();
        assert_eq!(harness.state().intents, vec![open()]);
        harness.state_mut().apply_intents();
        harness.run();
        assert!(
            says(&harness, "Opening the page"),
            "it asks for the page again"
        );

        harness.state_mut().shared = failed(sample::failure(FailureKind::Internal));
        harness.run();
        assert!(says(&harness, "The page could not be opened"));
        assert!(says(&harness, FailureKind::Internal.hint()));
        the_pager_of_a_failed_page_is_off(&mut harness);

        a_long_hint_leaves_the_button_in_view();

        let figure = Sample::FigureTop.piece(PieceKind::Figure);
        let mut gone = samples::page(Sample::FigureTop);
        gone.image = Some(ImageRef {
            path: "no-such-folder/page.png".into(),
        });
        let mut shared = samples::library(samples::catalogue());
        shared.apply_intent(open(), &mut Vec::new());
        samples::land_view(&mut shared, gone);
        harness.state_mut().shared = shared;
        samples::see_pictures(&mut harness);
        assert!(!has_picture_of(&harness, Sample::FigureTop));
        assert!(
            !is_spinning_for(&harness, PICTURE_ALT),
            "a picture that is gone is not a wait"
        );
        assert!(harness.query_by_label("Zoom in").is_none());
        testkit::save_png(&mut harness, "source-picture-missing");
        harness.get_by_role_and_label(Role::Tab, "Figures").click();
        samples::see_pictures(&mut harness);
        let name = figure
            .label
            .as_deref()
            .expect("the sample figure has a label");
        let picture = harness.query_by_role_and_label(Role::Image, name);
        assert!(picture.is_some(), "the other tabs still work");
    }
}
