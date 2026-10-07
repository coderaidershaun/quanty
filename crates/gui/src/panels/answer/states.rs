//! The words for every state with no result to list, and the line that stands over the list
//! while the answer is still on its way.

use eframe::egui;

use super::pane::Pane;
use super::phase::{Phase, Written};
use crate::contract::{Failure, Loadable};
use crate::theme::{Icon, TextRole};
use crate::widgets::{self, Placeholder};

const NO_ITEMS_HINT: &str =
    "Your library holds no items yet. Add a chapter on the Ingest tab, then ask again.";
const NO_ITEM_MATCHES_HINT: &str =
    "No item in your library matches the filters of this question. Clear a filter, then ask again.";

/// The whole body, for a search that has no result to show.
pub(super) fn whole(ui: &mut egui::Ui, pane: &Pane<'_, '_>) {
    match pane.phase {
        Phase::Idle | Phase::NoResults if library_is_empty(pane) => {
            Placeholder::empty(Icon::LIBRARY, "Your library is empty")
                .hint("Add a chapter on the Ingest tab, then ask about it here.")
                .show(ui);
        }
        Phase::Idle => {
            Placeholder::empty(Icon::SEARCH, "Ask your books")
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
        Phase::NoResults if no_document_has_the_labels(pane) => {
            Placeholder::empty(Icon::TAG, "No document has these labels")
                .hint("Change or remove a filter beside the question, then ask again.")
                .show(ui);
        }
        Phase::NoResults => {
            Placeholder::empty(Icon::SEARCH, "No results")
                .hint(why_nothing_was_found(pane))
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

/// A search has no score limit, so it finds nothing only when the library holds no items or the
/// filters of the question leave none. A search that had a filter reports how many documents
/// matched it.
fn why_nothing_was_found(pane: &Pane<'_, '_>) -> &'static str {
    let had_a_filter = pane
        .ask
        .search
        .ready()
        .is_some_and(|reply| reply.trace.documents_searched.is_some());
    if had_a_filter {
        NO_ITEM_MATCHES_HINT
    } else {
        NO_ITEMS_HINT
    }
}

/// A filter that matched no document: the search stopped before it looked at any item.
fn no_document_has_the_labels(pane: &Pane<'_, '_>) -> bool {
    pane.ask
        .search
        .ready()
        .is_some_and(|reply| reply.trace.documents_searched == Some(0))
}
