//! The Concepts tab: the concepts of the page as chips, and what it says while they load.

use eframe::egui;

use super::states::whole_area;
use super::tabs::SourceTab;
use crate::panels::labels::TRY_AGAIN;
use crate::panels::{PanelCx, PanelWidth};
use crate::theme::{Icon, Kind, TextRole};
use crate::widgets::{Chip, Placeholder};
use contract::{ConceptId, Failure, Intent, Loadable, PageConcept};

pub(super) const FAILED_TITLE: &str = "The concepts could not be read";

pub(super) fn show(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, width: PanelWidth) {
    let shared = cx.shared;
    match &shared.source.concepts {
        Loadable::Loading => {
            whole_area(
                ui,
                Placeholder::loading("Finding the concepts on this page"),
            );
        }
        Loadable::Failed(failure) => failed(ui, failure, width, cx.intents),
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
        Loadable::Ready(list) => chips(ui, list, width, cx),
    }
}

fn failed(ui: &mut egui::Ui, failure: &Failure, width: PanelWidth, intents: &mut Vec<Intent>) {
    let placeholder = Placeholder::error(FAILED_TITLE)
        .hint(&failure.hint)
        .action(TRY_AGAIN)
        .action_size(width.control_size());
    let shown = whole_area(ui, placeholder);
    shown.response.on_hover_text(&failure.detail);
    if shown.action_clicked {
        intents.push(Intent::ReloadSource);
    }
}

fn chips(ui: &mut egui::Ui, list: &[PageConcept], width: PanelWidth, cx: &mut PanelCx<'_>) {
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
                    let chip = Chip::kind(Kind::Concept, &concept.name)
                        .selected(is_focused)
                        .min_height(width.chip_height());
                    let mut response = ui.add(chip);
                    if !concept.definition.is_empty() {
                        response = response.on_hover_ui(|ui| {
                            ui.label(TextRole::Small.rich(ui, &concept.definition));
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
