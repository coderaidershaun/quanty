//! The Concepts tab: the concepts of the page as chips, and what it says while they load.

use eframe::egui;

use super::states::whole_area;
use super::tabs::SourceTab;
use crate::contract::{ConceptId, Failure, Intent, Loadable, PageConcept};
use crate::panels::PanelCx;
use crate::theme::{Icon, Kind, TextRole};
use crate::widgets::{Chip, Placeholder};

pub(super) fn show(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    let shared = cx.shared;
    match &shared.source.concepts {
        Loadable::Loading => {
            whole_area(
                ui,
                Placeholder::loading("Finding the concepts on this page"),
            );
        }
        Loadable::Failed(failure) => failed(ui, failure, cx.intents),
        Loadable::Idle => {
            whole_area(
                ui,
                Placeholder::empty(Icon::CONCEPT, "Concepts are not loaded"),
            );
        }
        Loadable::Ready(list) if list.is_empty() => {
            let title = "No concepts were found on this page";
            whole_area(ui, Placeholder::empty(Icon::CONCEPT, title));
        }
        Loadable::Ready(list) => chips(ui, list, cx),
    }
}

fn failed(ui: &mut egui::Ui, failure: &Failure, intents: &mut Vec<Intent>) {
    let placeholder = Placeholder::error("The concepts could not be read")
        .hint(&failure.hint)
        .action("Try again");
    let shown = whole_area(ui, placeholder);
    shown.response.on_hover_text(&failure.detail);
    if shown.action_clicked {
        intents.push(Intent::ReloadSource);
    }
}

fn chips(ui: &mut egui::Ui, list: &[PageConcept], cx: &mut PanelCx<'_>) {
    let nav = &cx.shared.source;
    let focused = cx.shared.ask.focused_concept;
    let page = nav.target.map(|target| target.page);
    egui::ScrollArea::vertical()
        .id_salt((SourceTab::Concepts, nav.generation, page))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for concept in list {
                    let is_focused = focused == Some(concept.id);
                    let chip = Chip::kind(Kind::Concept, &concept.name).selected(is_focused);
                    let mut response = ui.add(chip);
                    if !concept.definition.is_empty() {
                        response = response.on_hover_ui(|ui| {
                            ui.label(TextRole::Small.rich(&concept.definition));
                        });
                    }
                    if response.clicked() {
                        cx.intents.push(focus(concept.id, is_focused));
                    }
                }
            });
        });
}

fn focus(concept: ConceptId, is_focused: bool) -> Intent {
    Intent::FocusConcept((!is_focused).then_some(concept))
}
