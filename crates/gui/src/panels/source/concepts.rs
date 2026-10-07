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

/// One chip for each concept. A click focuses the concept, and a click on the focused one lets
/// go of it. The definition shows while the pointer is over the chip.
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

#[cfg(test)]
mod tests {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::Queryable;

    use crate::contract::{FailureKind, Intent, Loadable};
    use crate::panels::source::samples::{
        self, DEFAULT_SIZE as SIZE, Sample, has_picture_of, shows,
    };
    use crate::testkit::{self, sample};

    #[test]
    fn concept_chips_focus_a_concept_and_their_states_leave_the_page_alone() {
        let concepts = sample::page_concepts();
        let mut harness = samples::panel(SIZE, samples::opened(Sample::EveryKind, None));
        samples::see_pictures(&mut harness);
        harness.get_by_role_and_label(Role::Tab, "Concepts").click();
        harness.run();
        assert!(
            concepts
                .iter()
                .all(|concept| shows(&harness, &concept.name))
        );
        testkit::save_png(&mut harness, "source-concepts");
        harness.get_by_label(&concepts[0].name).click();
        harness.run();
        assert_eq!(
            harness.state().intents,
            vec![Intent::FocusConcept(Some(concepts[0].id))]
        );

        harness.state_mut().intents.clear();
        harness.state_mut().shared.ask.focused_concept = Some(concepts[0].id);
        harness.run();
        harness.get_by_label(&concepts[0].name).click();
        harness.run();
        assert_eq!(
            harness.state().intents,
            vec![Intent::FocusConcept(None)],
            "a click on the focused concept lets go of it"
        );
        harness.state_mut().intents.clear();

        harness.state_mut().shared.source.concepts = Loadable::Loading;
        harness.run();
        assert!(shows(&harness, "Finding the concepts on this page"));
        testkit::save_png(&mut harness, "source-concepts-loading");

        let failure = sample::failure(FailureKind::Internal);
        harness.state_mut().shared.source.concepts = Loadable::Failed(failure.clone());
        harness.run();
        assert!(shows(&harness, "The concepts could not be read"));
        assert!(shows(&harness, &failure.hint));
        testkit::save_png(&mut harness, "source-concepts-failed");
        harness.get_by_label("Try again").click();
        harness.run();
        assert_eq!(harness.state().intents, vec![Intent::ReloadSource]);
        harness.state_mut().intents.clear();
        harness.get_by_role_and_label(Role::Tab, "Page").click();
        harness.run();
        assert!(
            has_picture_of(&harness, Sample::EveryKind),
            "a failed read leaves the page alone"
        );
        harness.get_by_role_and_label(Role::Tab, "Concepts").click();

        harness.state_mut().shared.source.concepts = Loadable::Ready(Vec::new());
        harness.run();
        assert!(shows(&harness, "No concepts were found on this page"));
        testkit::save_png(&mut harness, "source-concepts-none");

        harness.state_mut().shared.source.concepts = Loadable::Idle;
        harness.run();
        assert!(shows(&harness, "Concepts are not loaded"));
    }
}
