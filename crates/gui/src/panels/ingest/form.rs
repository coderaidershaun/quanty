//! The fields of the form: the file, the media, and the tags of the PDF, with the fields and the
//! buttons that save a new media without a file or give it up.

use eframe::egui;

use super::Local;
use super::media::{self, MediaChoice, NewMediaForm};
use crate::contract::{Catalogue, Category, Intent, Loadable, Media};
use crate::panels::PanelCx;
use crate::panels::labels::{AUTHORS, LIST_PLACEHOLDER, TAGS, caption, category_and_authors};
use crate::state::MediaSave;
use crate::theme::{TextRole, color, space};
use crate::widgets::{Badge, Button, ControlSize, Dropdown, Notice, TextInput};

const MEDIA: &str = "Media";
const ADD_NEW_MEDIA: &str = "Add new media…";
const CATEGORY: &str = "Category";
const MEDIA_TITLE: &str = "Media title";
pub(super) const OWN_TAGS: &str = "Tags for this PDF";

const SAVE_NOTE: &str =
    "A saved media stays in this list with its authors and tags, also before its first PDF.";

pub(super) const RULE: &str =
    "A book's file must be named chapter-<number>-<name>.pdf, for example chapter-3-greeks.pdf.";
pub(super) const COST: &str = "Checking is free. Starting is paid work: claude and Jev convert each page, Gemini embeds the items, and claude reads the concepts.";

pub(super) fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let is_running = cx.shared.ingest.is_running();
    ui.label(TextRole::Heading.rich("Add media"));
    ui.add_space(space::MD);
    ui.add_enabled_ui(!is_running, |ui| {
        file_row(ui, local, cx.intents);
        if needs_a_chapter_name(&local.media) {
            ui.label(TextRole::Small.rich(RULE));
        }
        ui.add_space(space::MD);
        media_row(ui, local, cx);
        caption(ui, OWN_TAGS);
        TextInput::new(OWN_TAGS, &mut local.own_tags)
            .id_salt("ingest_own_tags")
            .placeholder("Optional. The media's tags apply as well.")
            .show(ui);
    });
    ui.add_space(space::SM);
    ui.label(TextRole::Small.rich(COST));
    ui.add_space(space::LG);
}

/// The file-name rule is for a book, and for a form that has no media yet.
fn needs_a_chapter_name(choice: &MediaChoice) -> bool {
    let category = match choice {
        MediaChoice::Unchosen => return true,
        MediaChoice::Existing { category, .. } => *category,
        MediaChoice::New(form) => form.category,
    };
    category == Category::Book
}

fn file_row(ui: &mut egui::Ui, local: &Local, intents: &mut Vec<Intent>) {
    ui.horizontal(|ui| {
        if ui.add(Button::secondary("Choose a PDF")).clicked() {
            intents.push(Intent::PickPdf);
        }
        let name = local
            .pdf
            .as_deref()
            .and_then(|pdf| pdf.file_name())
            .map(|name| name.to_string_lossy().into_owned());
        let (words, tint) = match name {
            Some(name) => (name, color::TEXT),
            None => ("No file chosen".to_owned(), color::TEXT_MUTED),
        };
        // No line height is set: a line taller than the font leaves the words above the middle
        // of the row.
        let shown = egui::RichText::new(words)
            .font(TextRole::Body.font())
            .color(tint);
        ui.add(egui::Label::new(shown).truncate());
    });
}

fn media_row(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    caption(ui, MEDIA);
    let catalogue = &cx.shared.library.catalogue;
    let offers = catalogue.ready().map(media::offers).unwrap_or_default();
    if offers.is_empty() {
        ui.label(TextRole::Small.rich(why_no_media_is_offered(catalogue)));
    }
    if let MediaChoice::New(form) = &mut local.media {
        new_media(ui, form);
        save_row(ui, local, cx);
        return;
    }
    list(ui, local, &offers);
    if let MediaChoice::Existing { title, .. } = &local.media
        && let Some(chosen) = offers
            .iter()
            .find(|media| media.title.as_deref() == Some(title.as_str()))
    {
        labels_of(ui, chosen);
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
    let mut rows: Vec<&str> = offers
        .iter()
        .filter_map(|media| media.title.as_deref())
        .collect();
    rows.push(ADD_NEW_MEDIA);
    let chosen = match &local.media {
        MediaChoice::Existing { title, .. } => offers
            .iter()
            .position(|media| media.title.as_deref() == Some(title.as_str())),
        _ => None,
    };
    // A library that failed to load again must not hide the media that was chosen.
    let placeholder = match &local.media {
        MediaChoice::Existing { title, .. } if chosen.is_none() => title,
        _ => "Choose a media",
    };
    let picked = Dropdown::new(MEDIA, &rows)
        .id_salt("ingest_media_list")
        .selected(chosen)
        .placeholder(placeholder)
        .size(ControlSize::Medium)
        .width(ui.available_width())
        .show(ui);
    match picked.map(|index| offers.get(index)) {
        Some(Some(media)) => local.choose(media),
        Some(None) => local.start_new_media(),
        None => {}
    }
}

/// The labels of a stored media are read only here: they come from the media, not from the form.
fn labels_of(ui: &mut egui::Ui, media: &Media) {
    ui.add_space(space::XS);
    ui.horizontal_wrapped(|ui| {
        ui.label(
            TextRole::Small
                .rich(category_and_authors(media))
                .color(color::TEXT_MUTED),
        );
        for tag in &media.tags {
            ui.add(Badge::new(tag));
        }
    });
}

fn new_media(ui: &mut egui::Ui, form: &mut NewMediaForm) {
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
    TextInput::new(MEDIA_TITLE, &mut form.title)
        .id_salt("ingest_media_title")
        .placeholder("The new media's title")
        .show(ui);
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

fn save_row(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let save = &cx.shared.library.media_save;
    let MediaChoice::New(form) = &local.media else {
        return;
    };
    let media = form.to_save();
    ui.add_space(space::MD);
    ui.horizontal(|ui| {
        let button = Button::secondary("Save media").loading(save.is_saving());
        if ui.add_enabled(media.is_some(), button).clicked()
            && let Some(media) = media.clone()
        {
            cx.intents.push(Intent::SaveMedia(media));
        }
        if ui
            .add_enabled(!save.is_saving(), Button::secondary("Cancel"))
            .clicked()
        {
            local.cancel_new_media();
        }
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
}
