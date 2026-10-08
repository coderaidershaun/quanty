//! The words for every state with no result to list, and the line that stands over the list
//! while the answer is still on its way.

use eframe::egui;

use super::pane::Pane;
use super::phase::{Phase, Written};
use crate::contract::{Failure, Loadable, NothingFound};
use crate::theme::{Icon, TextRole};
use crate::widgets::{self, Placeholder};

/// The whole body, for a search that has no result to show.
pub(super) fn whole(ui: &mut egui::Ui, pane: &Pane<'_, '_>) {
    match pane.phase {
        Phase::Idle | Phase::NoResults(_) if library_is_empty(pane) => {
            Placeholder::empty(Icon::LIBRARY, "Your library is empty")
                .hint("Add a PDF on the Ingest tab, then ask about it here.")
                .show(ui);
        }
        Phase::Idle => {
            Placeholder::empty(Icon::SEARCH, "Ask your library")
                .hint("Write a question above. The answer comes with the pages it stands on.")
                .show(ui);
        }
        Phase::Searching => {
            Placeholder::loading("Searching your library")
                .hint("The results come first. The answer follows.")
                .show(ui);
        }
        Phase::SearchFailed(failure) => failed(ui, "The search failed", failure),
        Phase::SearchStopped => {
            Placeholder::empty(Icon::STOP, "Search stopped")
                .hint("Ask again when you are ready.")
                .show(ui);
        }
        // The filters are beside the question here, so the hint points at them.
        Phase::NoResults(NothingFound::NoDocumentHasTheLabels) => {
            Placeholder::empty(Icon::TAG, "No document has these labels")
                .hint("Change or remove a filter beside the question, then ask again.")
                .show(ui);
        }
        Phase::NoResults(
            reason @ (NothingFound::NoItemMatchesTheFilters | NothingFound::LibraryHoldsNoItems),
        ) => {
            Placeholder::empty(Icon::SEARCH, "No results")
                .hint(reason.hint())
                .show(ui);
        }
        Phase::Found { .. } => {}
    }
}

/// Why the answer is not shown, in one strip that stays above the results.
pub(super) fn line(ui: &mut egui::Ui, written: Written<'_>) {
    match written {
        Written::Ready(_) => {}
        Written::Writing => {
            ui.horizontal_wrapped(|ui| {
                widgets::spinner(ui, "Writing the answer");
                ui.label(TextRole::BodyStrong.rich("Writing the answer"));
                ui.label(
                    TextRole::Small
                        .rich("It can take up to two minutes. The results below are ready now."),
                );
            });
        }
        Written::Failed(failure) => {
            widgets::Notice::error("The answer could not be written")
                .body(&failure.hint)
                .show(ui)
                .response
                .on_hover_ui(|ui| detail(ui, failure));
        }
        Written::Empty => {
            widgets::Notice::info("The results do not answer the question")
                .body("They are the nearest items in your library. Try a follow-up question.")
                .show(ui);
        }
        Written::Stopped => {
            widgets::Notice::info("Answer stopped")
                .body("The results below stand. Ask again for the answer.")
                .show(ui);
        }
        Written::NotAsked => {
            widgets::Notice::info("No answer was asked for")
                .body("This ask was Results only. Choose Answer beside the question and ask again.")
                .show(ui);
        }
    }
}

fn failed(ui: &mut egui::Ui, title: &str, failure: &Failure) {
    Placeholder::error(title)
        .hint(&failure.hint)
        .show(ui)
        .response
        .on_hover_ui(|ui| detail(ui, failure));
}

fn detail(ui: &mut egui::Ui, failure: &Failure) {
    ui.label(TextRole::Small.rich(&failure.detail));
}

fn library_is_empty(pane: &Pane<'_, '_>) -> bool {
    matches!(
        &pane.cx.shared.library.catalogue,
        Loadable::Ready(catalogue) if catalogue.documents().next().is_none()
    )
}
