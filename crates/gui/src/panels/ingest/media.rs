//! The media section: the media of the PDF, chosen from the library and shown as a card whose
//! labels are fixed, or typed and saved first, and the pencil that opens the same form to change
//! the labels of the chosen one.

use eframe::egui;

use super::Local;
use crate::contract::{Catalogue, Category, Intent, Loadable, Media, MediaEdit, NewMedia};
use crate::panels::PanelCx;
use crate::panels::labels::{AUTHORS, LIST_PLACEHOLDER, TAGS, caption, list_of, text_of};
use crate::state::{MediaEditing, MediaSave, Shared};
use crate::theme::{Icon, TextRole, Tone, color, space};
use crate::widgets::{Badge, Button, Card, ControlSize, Dropdown, Notice, TextInput};

const MEDIA: &str = "Media";
const ADD_NEW_MEDIA: &str = "Add new media…";
const CATEGORY: &str = "Category";
const MEDIA_TITLE: &str = "Media title";
const EDIT_MEDIA: &str = "Edit media";
const SAVE_MEDIA: &str = "Save media";
const CANCEL: &str = "Cancel";

const SAVE_NOTE: &str =
    "A saved media stays in this list with its authors and tags, also before its first PDF.";
const EDIT_NOTE: &str = "The title names the media's folder, so it cannot change.";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) enum MediaChoice {
    #[default]
    Unchosen,
    /// A media of the library, with its title exactly as the library has it.
    Existing { title: String, category: Category },
    /// A media that is typed, which is saved before a PDF of it can be ingested.
    New(MediaForm),
    /// The pencil was pressed on the chosen media. `is_sent` is true once its Save was pressed.
    Editing {
        title: String,
        category: Category,
        form: MediaForm,
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct MediaForm {
    pub(super) category: Category,
    pub(super) title: String,
    pub(super) authors: String,
    pub(super) tags: String,
}

impl MediaForm {
    fn of(media: &Media) -> MediaForm {
        MediaForm {
            category: media.category,
            title: media.title.clone().unwrap_or_default(),
            authors: text_of(&media.authors),
            tags: text_of(&media.tags),
        }
    }

    fn to_save(&self) -> Option<NewMedia> {
        let title = self.title.trim();
        (!title.is_empty()).then(|| NewMedia {
            title: title.to_owned(),
            category: self.category,
            authors: list_of(&self.authors),
            tags: list_of(&self.tags),
        })
    }

    fn to_edit(&self, title: &str) -> MediaEdit {
        MediaEdit {
            title: title.to_owned(),
            category: self.category,
            authors: list_of(&self.authors),
            tags: list_of(&self.tags),
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
                let is_pencil_pressed = card(ui, chosen);
                if is_pencil_pressed {
                    local.media = MediaChoice::Editing {
                        title: chosen.title.clone().unwrap_or_default(),
                        category: chosen.category,
                        form: MediaForm::of(chosen),
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
    let picked = Dropdown::new(MEDIA, &rows)
        .id_salt("ingest_media_list")
        .selected(chosen)
        .placeholder(&placeholder)
        .size(ControlSize::Medium)
        .width(ui.available_width())
        .show(ui);
    match picked.map(|index| offers.get(index)) {
        Some(Some(media)) => local.choose(media),
        Some(None) => local.media = MediaChoice::New(MediaForm::default()),
        None => {}
    }
}

/// The labels of a chosen media are fixed here: they come from the library, and only the pencil
/// changes them. Returns true when the pencil was pressed.
fn card(ui: &mut egui::Ui, media: &Media) -> bool {
    ui.add_space(space::SM);
    let title = media.title.as_deref().unwrap_or_default();
    let shown = Card::new().show(ui, |ui| {
        let mut is_pressed = false;
        ui.horizontal(|ui| {
            ui.label(TextRole::BodyStrong.rich(title));
            ui.add(Badge::new(media.category.label()).tone(Tone::Blue));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                is_pressed = ui.add(Button::icon_only(Icon::EDIT, EDIT_MEDIA)).clicked();
            });
        });
        if !media.authors.is_empty() {
            let authors = media.authors.join(", ");
            ui.label(TextRole::Small.rich(authors).color(color::TEXT_MUTED));
        }
        if !media.tags.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for tag in &media.tags {
                    ui.add(Badge::new(tag));
                }
            });
        }
        is_pressed
    });
    shown.inner
}

/// The title box is off for an edit: the title names the media's folder and cannot change.
fn fields(ui: &mut egui::Ui, form: &mut MediaForm, is_title_fixed: bool) {
    caption(ui, CATEGORY);
    let rows: Vec<&str> = Category::ALL
        .iter()
        .map(|category| category.label())
        .collect();
    let chosen = Category::ALL
        .iter()
        .position(|category| *category == form.category);
    let picked = Dropdown::new(CATEGORY, &rows)
        .id_salt("ingest_category")
        .selected(chosen)
        .size(ControlSize::Medium)
        .width(ui.available_width())
        .show(ui);
    if let Some(category) = picked.and_then(|index| Category::ALL.get(index)) {
        form.category = *category;
    }
    caption(ui, MEDIA_TITLE);
    ui.add_enabled_ui(!is_title_fixed, |ui| {
        TextInput::new(MEDIA_TITLE, &mut form.title)
            .id_salt("ingest_media_title")
            .placeholder("The new media's title")
            .show(ui);
    });
    caption(ui, AUTHORS);
    TextInput::new(AUTHORS, &mut form.authors)
        .id_salt("ingest_authors")
        .placeholder(LIST_PLACEHOLDER)
        .show(ui);
    caption(ui, TAGS);
    TextInput::new(TAGS, &mut form.tags)
        .id_salt("ingest_media_tags")
        .placeholder(LIST_PLACEHOLDER)
        .show(ui);
}

fn new_media(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let MediaChoice::New(form) = &mut local.media else {
        return;
    };
    fields(ui, form, false);
    let save = &cx.shared.library.media_save;
    let media = form.to_save();
    ui.add_space(space::MD);
    let mut is_cancelled = false;
    ui.horizontal(|ui| {
        let button = Button::secondary(SAVE_MEDIA).loading(save.is_saving());
        if ui.add_enabled(media.is_some(), button).clicked()
            && let Some(media) = media.clone()
        {
            cx.intents.push(Intent::SaveMedia(media));
        }
        is_cancelled = ui
            .add_enabled(!save.is_saving(), Button::secondary(CANCEL))
            .clicked();
        ui.add(egui::Label::new(TextRole::Small.rich(SAVE_NOTE)).truncate());
    });
    // A person who changes the title no longer sees the refusal of the old one.
    if let MediaSave::Failed {
        media: failed,
        failure,
    } = save
        && media.is_some_and(|media| media.title == failed.title)
    {
        ui.add_space(space::SM);
        Notice::error(&failure.hint).show(ui);
    }
    // The chosen file and the fields of the PDF stay: they were not typed for the new media.
    if is_cancelled {
        local.media = MediaChoice::Unchosen;
    }
}

/// An edit rewrites every document of the media, so the state refuses it while an ingest runs
/// and while a media is saved or edited. Save is off then, so a press is never dropped.
fn can_edit(shared: &Shared) -> bool {
    !shared.ingest.is_running()
        && !shared.library.media_save.is_saving()
        && !shared.library.media_edit.is_saving()
}

fn edited_media(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let MediaChoice::Editing {
        title,
        category,
        form,
        is_sent,
    } = &mut local.media
    else {
        return;
    };
    fields(ui, form, true);
    let edit = form.to_edit(title);
    let media_edit = &cx.shared.library.media_edit;
    ui.add_space(space::MD);
    let mut is_cancelled = false;
    ui.horizontal(|ui| {
        let button = Button::secondary(SAVE_MEDIA).loading(*is_sent);
        if ui.add_enabled(can_edit(cx.shared), button).clicked() {
            *is_sent = true;
            cx.intents.push(Intent::EditMedia(edit.clone()));
        }
        is_cancelled = ui
            .add_enabled(!media_edit.is_saving(), Button::secondary(CANCEL))
            .clicked();
        ui.add(egui::Label::new(TextRole::Small.rich(EDIT_NOTE)).truncate());
    });
    // The refusal shows while the form still holds the edit that was refused.
    if let MediaEditing::Failed {
        edit: refused,
        failure,
    } = media_edit
        && *refused == edit
    {
        ui.add_space(space::SM);
        Notice::error(&failure.hint).show(ui);
    }
    if is_cancelled {
        local.media = MediaChoice::Existing {
            title: std::mem::take(title),
            category: *category,
        };
    }
}
