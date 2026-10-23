//! The form of a build: the name, the media, the documents held out, how much the models write,
//! and the dry run.

use eframe::egui;

use crate::panels::labels::caption;
use crate::state::Library;
use crate::theme::{TextRole, space};
use crate::widgets::{ControlSize, Dropdown, Step, Stepper, TextInput, paint_caret};
use contract::{DatasetRequest, DocId};

const NAME: &str = "Dataset name";
const NAME_PLACEHOLDER: &str = "Letters, digits, -, _ and .";
const MEDIA: &str = "Media";
const ALL_MEDIA: &str = "All media";
const HOLD_OUT: &str = "Hold out";
const NONE_HELD_OUT: &str = "None held out";
const DRY_RUN: &str = "Dry run (export, check and plan only; asks no model)";

/// The three boxes are this wide, so their edges line up.
const FIELD_WIDTH: f32 = 360.0;
/// The captions of the steppers are given this much room, so the three steppers line up.
const CAPTION_WIDTH: f32 = 200.0;

/// A whole number the person steps between 0 and `max`.
struct Counter {
    caption: &'static str,
    fewer: &'static str,
    more: &'static str,
    max: u32,
    by: u32,
}

const REWRITES: Counter = Counter {
    caption: "Rewrites per section",
    fewer: "Fewer rewrites per section",
    more: "More rewrites per section",
    max: 10,
    by: 1,
};
const QUESTIONS: Counter = Counter {
    caption: "Questions per section",
    fewer: "Fewer questions per section",
    more: "More questions per section",
    max: 10,
    by: 1,
};
const MAX_SFT: Counter = Counter {
    caption: "Most SFT examples",
    fewer: "Fewer SFT examples at most",
    more: "More SFT examples at most",
    max: 2000,
    by: 50,
};

/// What the Media and Hold out lists offer, built once for each revision of the library.
#[derive(Debug, Default)]
pub(super) struct Choices {
    /// The library revision these lists were built from. `None` means they are not built yet.
    revision: Option<u64>,
    /// True when the lists come from a catalogue that has loaded.
    is_known: bool,
    /// "All media" comes first, then each titled media that has a document, in catalogue order.
    media: Vec<String>,
    /// One list for each row of `media`: the documents that have a folder, with their labels. The
    /// row of "All media" holds those of every titled media, each labelled with its media, and the
    /// other rows label a document by its own name.
    documents: Vec<Vec<(DocId, String)>>,
}

impl Choices {
    /// True when the lists were built again.
    pub(super) fn refresh(&mut self, library: &Library) -> bool {
        if self.revision == Some(library.revision) {
            return false;
        }
        self.revision = Some(library.revision);
        let catalogue = library.catalogue.ready();
        self.is_known = catalogue.is_some();
        self.media = vec![ALL_MEDIA.to_owned()];
        self.documents = vec![Vec::new()];
        let with_documents = catalogue
            .into_iter()
            .flat_map(|catalogue| &catalogue.media)
            .filter(|media| !media.documents.is_empty());
        for media in with_documents {
            let Some(title) = media.title.as_deref() else {
                continue;
            };
            let with_folder = media
                .documents
                .iter()
                .filter(|document| document.folder.is_some());
            let mut own = Vec::new();
            for document in with_folder {
                let label = document.name().label();
                self.documents[0].push((document.id, format!("{title}, {label}")));
                own.push((document.id, label));
            }
            self.media.push(title.to_owned());
            self.documents.push(own);
        }
        true
    }

    /// The documents to offer while `media` is the chosen media, or none is.
    pub(super) fn documents_of(&self, media: Option<&str>) -> &[(DocId, String)] {
        let row = media
            .and_then(|media| self.media.iter().skip(1).position(|title| title == media))
            .map_or(0, |place| place + 1);
        self.documents.get(row).map_or(&[], Vec::as_slice)
    }

    /// With a known catalogue, a media that is not offered becomes "All media", and a held-out
    /// document that the chosen media does not offer is dropped. A build of either would fail or
    /// hold out nothing, and not say why.
    pub(super) fn keep_known(&self, draft: &mut DatasetRequest) {
        if !self.is_known {
            return;
        }
        draft
            .media
            .take_if(|media| !self.media.iter().skip(1).any(|title| title == media));
        let offered = self.documents_of(draft.media.as_deref());
        draft
            .holdout
            .retain(|held| offered.iter().any(|(id, _)| id == held));
    }
}

/// The draft with its name trimmed, or `None` while the name is blank. The rules for a name are
/// the dataset crate's, and a name it refuses comes back as a failure with a hint.
pub(super) fn request_of(draft: &DatasetRequest) -> Option<DatasetRequest> {
    let name = draft.name.trim();
    (!name.is_empty()).then(|| DatasetRequest {
        name: name.to_owned(),
        ..draft.clone()
    })
}

/// True when the person changed anything.
pub(super) fn show(ui: &mut egui::Ui, draft: &mut DatasetRequest, choices: &Choices) -> bool {
    caption(ui, NAME);
    let mut is_changed = TextInput::new(NAME, &mut draft.name)
        .placeholder(NAME_PLACEHOLDER)
        .width(FIELD_WIDTH)
        .show(ui)
        .response
        .changed();
    caption(ui, MEDIA);
    if pick_media(ui, draft, choices) {
        choices.keep_known(draft);
        is_changed = true;
    }
    caption(ui, HOLD_OUT);
    is_changed |= pick_held_out(ui, draft, choices);
    ui.add_space(space::MD);
    is_changed |= pick_count(ui, &REWRITES, &mut draft.rewrites);
    is_changed |= pick_count(ui, &QUESTIONS, &mut draft.questions);
    is_changed |= pick_count(ui, &MAX_SFT, &mut draft.max_sft);
    ui.add_space(space::SM);
    let response = ui.checkbox(&mut draft.dry_run, DRY_RUN);
    crate::widgets::interactive_cursor(ui, &response);
    is_changed |= response.changed();
    is_changed
}

/// Row 0 is "All media" and means every media. The list is off until a media gives it more rows.
fn pick_media(ui: &mut egui::Ui, draft: &mut DatasetRequest, choices: &Choices) -> bool {
    let selected = match draft.media.as_deref() {
        None => Some(0),
        Some(current) => choices.media.iter().position(|title| title == current),
    };
    ui.add_enabled_ui(choices.media.len() > 1, |ui| {
        // A media that is not in the list, because the library has not loaded, shows as the
        // placeholder.
        let shown = draft.media.as_deref().unwrap_or(ALL_MEDIA);
        let chosen = Dropdown::new(MEDIA, &choices.media, FIELD_WIDTH)
            .selected(selected)
            .placeholder(shown)
            .size(ControlSize::Medium)
            .show(ui);
        let Some(index) = chosen else {
            return false;
        };
        let picked = (index > 0).then(|| choices.media[index].clone());
        let is_changed = picked != draft.media;
        draft.media = picked;
        is_changed
    })
    .inner
}

/// The widget kit has no list where a person can tick several rows, so this one is a combo box
/// of check boxes. The ticked ids keep the order of the list.
fn pick_held_out(ui: &mut egui::Ui, draft: &mut DatasetRequest, choices: &Choices) -> bool {
    let offered = choices.documents_of(draft.media.as_deref());
    let summary = held_out_summary(&draft.holdout, offered);
    let mut is_changed = false;
    ui.add_enabled_ui(!offered.is_empty(), |ui| {
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = ControlSize::Medium.height();
            ui.set_max_width(FIELD_WIDTH);
            let combo = egui::ComboBox::from_id_salt("hold-out")
                .selected_text(summary)
                .width(FIELD_WIDTH)
                .truncate()
                .icon(paint_caret)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show_ui(ui, |ui| {
                    for (id, label) in offered {
                        let mut is_held = draft.holdout.contains(id);
                        let response = ui.checkbox(&mut is_held, label.as_str());
                        crate::widgets::interactive_cursor(ui, &response);
                        if response.changed() {
                            is_changed = true;
                            draft.holdout = offered
                                .iter()
                                .map(|(each, _)| *each)
                                .filter(|each| {
                                    if each == id {
                                        is_held
                                    } else {
                                        draft.holdout.contains(each)
                                    }
                                })
                                .collect();
                        }
                    }
                });
            crate::widgets::interactive_cursor(ui, &combo.response);
            ui.ctx()
                .accesskit_node_builder(combo.response.id, |node| node.set_label(HOLD_OUT));
        });
    });
    is_changed
}

/// What the closed box says. Ids that the list does not offer yet, such as before the catalogue
/// loads, count as held out documents without a name.
fn held_out_summary(held: &[DocId], offered: &[(DocId, String)]) -> String {
    match held {
        [] => NONE_HELD_OUT.to_owned(),
        [only] => offered.iter().find(|(id, _)| id == only).map_or_else(
            || "1 document held out".to_owned(),
            |(_, label)| label.clone(),
        ),
        many => format!("{} documents held out", many.len()),
    }
}

/// The caption in a cell of fixed width, then the stepper. True when the value changed.
fn pick_count(ui: &mut egui::Ui, counter: &Counter, value: &mut u32) -> bool {
    let step = ui
        .horizontal(|ui| {
            ui.allocate_ui(
                egui::vec2(CAPTION_WIDTH, ControlSize::Small.height()),
                |ui| {
                    // The size given above is only the most the cell may take, so its width is
                    // also set as the least.
                    ui.set_min_width(CAPTION_WIDTH);
                    ui.label(TextRole::Label.rich(counter.caption));
                },
            );
            Stepper::new(&value.to_string())
                .previous(counter.fewer, *value > 0)
                .next(counter.more, *value < counter.max)
                .show(ui)
        })
        .inner;
    match step {
        Some(Step::Previous) => *value = value.saturating_sub(counter.by),
        Some(Step::Next) => *value = (*value + counter.by).min(counter.max),
        None => return false,
    }
    true
}
