//! What the panel says when there is no page to show: nothing open, opening, or not opened.

use eframe::egui;

use crate::panels::labels::TRY_AGAIN;
use crate::panels::{PanelCx, PanelWidth};
use crate::theme::Icon;
use crate::widgets::{Button, ControlSize, Placeholder, PlaceholderResponse};
use contract::{Failure, FailureKind, Intent, Loadable};

/// In a room lower than this a placeholder sits in a scroll area, so none of it is cut off.
const MIN_ROOM: f32 = 200.0;

pub(super) fn show(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, width: PanelWidth) {
    match &cx.shared.source.page {
        Loadable::Loading => {
            whole_area(ui, Placeholder::loading("Opening the page"));
        }
        Loadable::Failed(failure) => failed(ui, failure, width.control_size(), cx),
        Loadable::Idle | Loadable::Ready(_) => nothing_open(ui, cx),
    }
}

fn nothing_open(ui: &mut egui::Ui, cx: &PanelCx<'_>) {
    let hint = match cx.shared.library.catalogue.ready() {
        Some(catalogue) if catalogue.documents().next().is_none() => {
            "The library is empty. Add a PDF on the Ingest tab to read it here."
        }
        // SMELL: a short and narrow panel folds the pickers behind the "Choose the document"
        // button, so there the media to choose stands above only once that button is pressed.
        Some(_) => "Select a result, or choose a media above, to see the page it stands on.",
        None => "Select a result to see the page it stands on.",
    };
    whole_area(
        ui,
        Placeholder::empty(Icon::DOCUMENT, "No page open").hint(hint),
    );
}

fn failed(ui: &mut egui::Ui, failure: &Failure, size: ControlSize, cx: &mut PanelCx<'_>) {
    let title = match failure.kind {
        FailureKind::SourceMissing => "The page was not found",
        _ => "The page could not be opened",
    };
    let placeholder = Placeholder::error(title).hint(&failure.hint);
    let is_retry_clicked = if ui.available_height() >= MIN_ROOM {
        let shown = whole_area(ui, placeholder.action(TRY_AGAIN).action_size(size));
        shown.response.on_hover_text(&failure.detail);
        shown.action_clicked
    } else {
        retry_below_scrolling_text(ui, placeholder, &failure.detail, size)
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
    size: ControlSize,
) -> bool {
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
        let retry = Button::secondary(TRY_AGAIN).size(size);
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
