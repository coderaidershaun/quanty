//! What the app says to a person while it works, each in every state: notices, the card of an area
//! that is empty, loading or failed, progress, and the sheet that asks before a change.

use eframe::egui;

use super::{State, column, section};
use crate::theme::{Icon, TextRole, size};
use crate::widgets::{
    Button, Choice, Confirm, Notice, Placeholder, indeterminate_bar, panel_frame, progress_bar,
    spinner,
};

const PLACEHOLDER_AREA: egui::Vec2 = egui::vec2(196.0, 170.0);

pub(super) fn notices(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Notices");
    let info = Notice::info("No sources found")
        .body(
            "I can't answer this question from the ingested documents. Try broadening the \
             question or adding more relevant documents.",
        )
        .dismissable()
        .show(ui);
    if info.dismissed {
        state.last_activated = Some("Notice::dismiss");
    }
    let warning = Notice::warning("Qdrant is not running")
        .body("Start it with docker compose up, then try again.")
        .action("Retry")
        .show(ui);
    if warning.action_clicked {
        state.last_activated = Some("Notice::action");
    }
    Notice::error("The answer could not be written")
        .body("Claude reached its usage limit. Try again after it resets.")
        .show(ui);
    Notice::success("PDF ingested").show(ui);
}

pub(super) fn placeholders(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Placeholders");
    ui.horizontal_top(|ui| {
        let areas = [
            Placeholder::empty(Icon::SEARCH, "Nothing here yet")
                .hint("Ask a question to see the results.")
                .action("Ask a question"),
            Placeholder::loading("Loading the page").hint("This takes a moment."),
            Placeholder::error("The page could not be read")
                .hint("Check that the document's folder exists.")
                .action("Try again"),
        ];
        for placeholder in areas {
            let shown = panel_frame()
                .show(ui, |ui| {
                    ui.set_min_size(PLACEHOLDER_AREA);
                    ui.set_max_size(PLACEHOLDER_AREA);
                    placeholder.show(ui)
                })
                .inner;
            if shown.action_clicked {
                state.last_activated = Some("Placeholder::action");
            }
        }
    });
}

pub(super) fn progress(ui: &mut egui::Ui) {
    section(ui, "Progress");
    ui.horizontal(|ui| {
        spinner(ui, "Working");
        ui.label(TextRole::Small.rich("Working"));
    });
    column(ui, 360.0, 0.0, |ui| {
        progress_bar(ui, "Reading the page", 0.4);
        progress_bar(ui, "Ingest finished", 1.0);
        indeterminate_bar(ui, "Checking the PDF");
    });
}

pub(super) fn confirm(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Confirm");
    let open = ui.add(Button::secondary("Open the dialog"));
    if open.clicked() {
        state.demo.is_confirm_open = true;
    }
    if state.demo.is_confirm_open {
        let sheet = Confirm::new(egui::Id::new("gallery-confirm"), "Delete this document?")
            .body("Its pages are removed from the library. This cannot be undone.")
            .confirm_label("Delete document")
            .destructive()
            .show(ui.ctx());
        match sheet {
            Some(Choice::Confirmed) => {
                state.demo.is_confirm_open = false;
                state.last_activated = Some("Confirm::confirmed");
            }
            Some(Choice::Cancelled) => {
                state.demo.is_confirm_open = false;
                state.last_activated = Some("Confirm::cancelled");
            }
            None => {}
        }
    }
    ui.add_space(size::CONTROL_LG);
}
