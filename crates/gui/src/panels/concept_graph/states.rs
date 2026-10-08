//! What the panel says when there is no picture: nothing asked yet, a wait, a failure, or a
//! graph with nothing to relate.

use eframe::egui;

use crate::contract::Loadable;
use crate::panels::PanelCx;
use crate::theme::Icon;
use crate::widgets::Placeholder;

pub(super) fn show(ui: &mut egui::Ui, cx: &PanelCx<'_>) {
    let ask = &cx.shared.ask;
    match &ask.graph {
        Loadable::Idle if ask.question.is_empty() => {
            Placeholder::empty(Icon::GRAPH, "No graph yet")
                .hint("Ask a question to see how its concepts relate.")
                .show(ui);
        }
        Loadable::Idle => {
            let hint = if ask.search.failure().is_some() {
                "The search failed, so there is nothing to relate."
            } else {
                "The search was stopped."
            };
            Placeholder::empty(Icon::GRAPH, "No graph for this question")
                .hint(hint)
                .show(ui);
        }
        Loadable::Loading => {
            Placeholder::loading("Reading the concept graph").show(ui);
        }
        Loadable::Failed(failure) => {
            Placeholder::error("The concept graph could not be read")
                .hint(&failure.hint)
                .show(ui)
                .response
                .on_hover_text(&failure.detail);
        }
        Loadable::Ready(_) => ready_without_concepts(ui, cx),
    }
}

fn ready_without_concepts(ui: &mut egui::Ui, cx: &PanelCx<'_>) {
    if cx.shared.ask.results().is_empty() {
        Placeholder::empty(Icon::GRAPH, "Nothing to relate")
            .hint("The search found no sources.")
            .show(ui);
    } else {
        Placeholder::empty(Icon::GRAPH, "No concepts for these results")
            .hint("The items that were found have no stored concepts.")
            .show(ui);
    }
}
