//! The media section: the media of the PDF, chosen from the library and shown as a card whose
//! labels are fixed, or typed and saved first, and the pencil that opens the same form to change
//! the labels of the chosen one.

use eframe::egui;

use super::Local;
use crate::contract::{Catalogue, Category, Failure, Intent, Loadable, Media, MediaEdit};
use crate::panels::PanelCx;
use crate::panels::labels::caption;
use crate::panels::media_card::{
    self, FormSetup, MediaFields, Pencil, Pressed, Purpose, Save, would_change,
};
use crate::state::{MediaEditing, MediaSave};
use crate::theme::{TextRole, space};
use crate::widgets::{Card, ControlSize, Dropdown};

const MEDIA: &str = "Media";
const ADD_NEW_MEDIA: &str = "Add new media…";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) enum MediaChoice {
    #[default]
    Unchosen,
    /// A media of the library, with its title exactly as the library has it.
    Existing { title: String, category: Category },
    /// A media that is typed, which is saved before a PDF of it can be ingested.
    New(MediaFields),
    /// The pencil was pressed on the chosen media. `category` is the stored one and `fields` hold
    /// what is typed, so a chip that is clicked changes neither the draft nor the check. `is_sent`
    /// is true once its Save was pressed.
    Editing {
        title: String,
        category: Category,
        fields: MediaFields,
        is_sent: bool,
    },
}

impl MediaChoice {
    /// The media a PDF goes into: a media of the library, also while its labels are edited.
    pub(super) fn chosen(&self) -> Option<(&str, Category)> {
        match self {
            MediaChoice::Existing { title, category }
            | MediaChoice::Editing {
                title, category, ..
            } => Some((title, *category)),
            MediaChoice::Unchosen | MediaChoice::New(_) => None,
        }
    }
}

/// The comparison is exact, so that a library which already holds two titles that differ only in
/// capitals still offers each of them.
pub(super) fn titled<'a>(catalogue: &'a Catalogue, title: &str) -> Option<&'a Media> {
    catalogue
        .media
        .iter()
        .find(|media| media.title.as_deref() == Some(title))
}

/// Every titled media, also one with no document yet.
fn offers(catalogue: &Catalogue) -> Vec<&Media> {
    catalogue
        .media
        .iter()
        .filter(|media| media.title.is_some())
        .collect()
}

fn place_of_chosen(media: &MediaChoice, offers: &[&Media]) -> Option<usize> {
    let MediaChoice::Existing { title, .. } = media else {
        return None;
    };
    offers
        .iter()
        .position(|offer| offer.title.as_deref() == Some(title.as_str()))
}

fn row_of(title: &str, category: Category) -> String {
    format!("{title} · {}", category.label())
}

pub(super) fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    caption(ui, MEDIA);
    let catalogue = &cx.shared.library.catalogue;
    let offers = catalogue.ready().map(offers).unwrap_or_default();
    if offers.is_empty() {
        ui.label(TextRole::Small.rich(why_no_media_is_offered(catalogue)));
    }
    match &local.media {
        MediaChoice::New(_) => new_media(ui, local, cx),
        MediaChoice::Editing { .. } => edited_media(ui, local, cx),
        MediaChoice::Unchosen | MediaChoice::Existing { .. } => {
            list(ui, local, &offers);
            let chosen = place_of_chosen(&local.media, &offers).and_then(|place| offers.get(place));
            if let Some(chosen) = chosen {
                let pencil = if cx.shared.can_edit_media() {
                    Pencil::On
                } else {
                    Pencil::Off
                };
                if card(ui, chosen, pencil) {
                    local.media = MediaChoice::Editing {
                        title: chosen.title.clone().unwrap_or_default(),
                        category: chosen.category,
                        fields: MediaFields::of(chosen),
                        is_sent: false,
                    };
                }
            }
        }
    }
}

fn why_no_media_is_offered(catalogue: &Loadable<Catalogue>) -> &str {
    match catalogue {
        Loadable::Idle | Loadable::Loading => "The library is still loading.",
        Loadable::Failed(failure) => &failure.hint,
        Loadable::Ready(catalogue) if catalogue.documents().next().is_none() => {
            "No media is stored yet."
        }
        Loadable::Ready(_) => "No stored document names its media. Add new media.",
    }
}

fn list(ui: &mut egui::Ui, local: &mut Local, offers: &[&Media]) {
    let mut rows: Vec<String> = offers
        .iter()
        .filter_map(|media| Some(row_of(media.title.as_deref()?, media.category)))
        .collect();
    rows.push(ADD_NEW_MEDIA.to_owned());
    let chosen = place_of_chosen(&local.media, offers);
    // A library that failed to load again must not hide the media that was chosen.
    let placeholder = match &local.media {
        MediaChoice::Existing { title, category } if chosen.is_none() => row_of(title, *category),
        _ => "Choose a media".to_owned(),
    };
    let picked = Dropdown::new(MEDIA, &rows, ui.available_width())
        .id_salt("ingest_media_list")
        .selected(chosen)
        .placeholder(&placeholder)
        .size(ControlSize::Medium)
        .show(ui);
    match picked.map(|index| offers.get(index)) {
        Some(Some(media)) => local.choose(media),
        Some(None) => local.media = MediaChoice::New(MediaFields::default()),
        None => {}
    }
}

/// The labels of a chosen media are fixed here: they come from the library, and only the pencil
/// changes them. Returns true when the pencil was pressed.
fn card(ui: &mut egui::Ui, media: &Media, pencil: Pencil) -> bool {
    ui.add_space(space::SM);
    let title = media.title.as_deref().unwrap_or_default();
    let shown = Card::new().show(ui, |ui| {
        let is_pressed = media_card::title_line(ui, title, media.category, |ui| {
            media_card::pencil(ui, title, pencil)
        });
        media_card::labels_line(ui, media);
        is_pressed
    });
    shown.inner
}

fn new_media(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let MediaChoice::New(fields) = &mut local.media else {
        return;
    };
    let media_save = &cx.shared.library.media_save;
    let typed = fields.to_new();
    // A person who changes the title no longer sees the refusal of the old one.
    let typed_title = typed.as_ref().map(|typed| typed.title.as_str());
    let refusal = match media_save {
        MediaSave::Failed { media, failure } if typed_title == Some(&media.title) => Some(failure),
        _ => None,
    };
    // SMELL: this copies a part of the rule by which the state takes a save of a media. The state
    // also drops a save while an edit of a media is on its way, and this button stays on then.
    let save = if media_save.is_saving() {
        Save::Sent
    } else if typed.is_some() && !cx.shared.library.is_deleting() {
        Save::On
    } else {
        Save::Off
    };
    let setup = FormSetup {
        id_salt: "ingest_new_media",
        purpose: Purpose::New,
        save,
        refusal,
    };
    match media_card::media_form(ui, fields, setup) {
        Some(Pressed::Save) => {
            if let Some(media) = fields.to_new() {
                cx.intents.push(Intent::SaveMedia(media));
            }
        }
        // The chosen file and the fields of the PDF stay: they were not typed for the new media.
        Some(Pressed::Cancel) => local.media = MediaChoice::Unchosen,
        None => {}
    }
}

/// The refusal shows while the form still holds the edit that was refused.
fn refusal_of<'a>(media_edit: &'a MediaEditing, edit: &MediaEdit) -> Option<&'a Failure> {
    match media_edit {
        MediaEditing::Failed {
            edit: refused,
            failure,
        } if refused == edit => Some(failure),
        _ => None,
    }
}

fn edited_media(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let MediaChoice::Editing {
        title,
        category,
        fields,
        is_sent,
    } = &mut local.media
    else {
        return;
    };
    let library = &cx.shared.library;
    let edit = fields.to_edit();
    let stored = library
        .catalogue
        .ready()
        .and_then(|catalogue| titled(catalogue, title));
    let can_save =
        stored.is_some_and(|stored| would_change(&edit, stored)) && cx.shared.can_edit_media();
    let save = if *is_sent {
        Save::Sent
    } else if can_save {
        Save::On
    } else {
        Save::Off
    };
    let refusal = refusal_of(&library.media_edit, &edit);
    ui.add_space(space::SM);
    let shown = Card::new().show(ui, |ui| {
        media_card::title_line(ui, title, *category, |_| {});
        let setup = FormSetup {
            id_salt: "ingest_media",
            purpose: Purpose::Edit,
            save,
            refusal,
        };
        media_card::media_form(ui, fields, setup)
    });
    match shown.inner {
        Some(Pressed::Save) => {
            *is_sent = true;
            cx.intents.push(Intent::EditMedia(fields.to_edit()));
        }
        Some(Pressed::Cancel) => {
            local.media = MediaChoice::Existing {
                title: std::mem::take(title),
                category: *category,
            };
        }
        None => {}
    }
}
