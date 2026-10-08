//! What the Ingest and the Library pages draw alike for a media: its title line with the category
//! badge and the pencil, the line of its authors and tags, and the form that saves a new media or
//! changes a stored one. Each page keeps its own state and sends its own intents.

use eframe::egui;

use super::labels::{AUTHORS, LIST_PLACEHOLDER, TAGS, caption, list_of, text_of};
use crate::contract::{Category, Failure, Media, MediaEdit, NewMedia};
use crate::theme::{Icon, TextRole, Tone, color, space};
use crate::widgets::{Badge, Button, Chip, ControlSize, Notice, TextInput};

pub(super) const NO_MEDIA: &str = "No media";

const EDIT: &str = "Edit";
const CATEGORY: &str = "Category";
const MEDIA_TITLE: &str = "Media title";
const SAVE_MEDIA: &str = "Save media";
const CANCEL: &str = "Cancel";

const NEW_NOTE: &str =
    "A saved media stays in this list with its authors and tags, also before its first PDF.";
const EDIT_NOTE: &str = "The title names the media's folder, so it cannot change.";

/// What the form of a media holds as typed. For an edit, the title is the library's own.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct MediaFields {
    pub(super) category: Category,
    pub(super) title: String,
    pub(super) authors: String,
    pub(super) tags: String,
}

impl MediaFields {
    pub(super) fn of(media: &Media) -> MediaFields {
        MediaFields {
            category: media.category,
            title: media.title.clone().unwrap_or_default(),
            authors: text_of(&media.authors),
            tags: text_of(&media.tags),
        }
    }

    /// `None` while the title is blank.
    pub(super) fn to_new(&self) -> Option<NewMedia> {
        let title = self.title.trim();
        (!title.is_empty()).then(|| NewMedia {
            title: title.to_owned(),
            category: self.category,
            authors: list_of(&self.authors),
            tags: list_of(&self.tags),
        })
    }

    /// The title is sent as the library has it, because it names the media.
    pub(super) fn to_edit(&self) -> MediaEdit {
        MediaEdit {
            title: self.title.clone(),
            category: self.category,
            authors: list_of(&self.authors),
            tags: list_of(&self.tags),
        }
    }
}

/// The stores keep the tags in lower case, so "Options" for a stored "options" is no change. A
/// repeated or reordered author or tag counts as a change, and saving it is harmless.
pub(super) fn would_change(edit: &MediaEdit, media: &Media) -> bool {
    let tags: Vec<String> = edit.tags.iter().map(|tag| tag.to_lowercase()).collect();
    edit.category != media.category || edit.authors != media.authors || tags != media.tags
}

/// The pencil at the end of a title line: not there while the form of its media is open, and off
/// while an edit of a media cannot be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pencil {
    Hidden,
    Off,
    On,
}

/// The first line of the card of a media: its title, its category and, at the right end, its
/// pencil. Returns true when the pencil was pressed.
pub(super) fn title_line(
    ui: &mut egui::Ui,
    title: &str,
    category: Category,
    pencil: Pencil,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(TextRole::BodyStrong.rich(title));
        ui.add(category_badge(category));
        if pencil == Pencil::Hidden {
            return false;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let name = format!("Edit {title}");
            let button = Button::secondary(EDIT)
                .icon(Icon::EDIT)
                .size(ControlSize::Small)
                .accessible_name(&name);
            ui.add_enabled(pencil == Pencil::On, button).clicked()
        })
        .inner
    })
    .inner
}

fn category_badge(category: Category) -> Badge<'static> {
    let (tone, icon) = match category {
        Category::Book => (Tone::Blue, Icon::BOOK),
        Category::Paper => (Tone::Purple, Icon::DOCUMENT),
        Category::Other => (Tone::Neutral, Icon::FOLDER),
    };
    Badge::new(category.label()).tone(tone).icon(icon)
}

/// The authors and the tags of a media on one line that wraps, and nothing when it has neither.
pub(super) fn labels_line(ui: &mut egui::Ui, media: &Media) {
    if media.authors.is_empty() && media.tags.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        if !media.authors.is_empty() {
            let authors = media.authors.join(", ");
            ui.label(TextRole::Small.rich(authors).color(color::TEXT_SECONDARY));
        }
        for tag in &media.tags {
            ui.add(Badge::new(tag));
        }
    });
}

/// Only a new media has a box for its title: the title of a stored media names its folder and
/// cannot change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Purpose {
    New,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pressed {
    Save,
    Cancel,
}

/// `Sent` is a save on its way, which holds the form still until it is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Save {
    Off,
    On,
    Sent,
}

pub(super) struct FormSetup<'a> {
    /// Keeps the boxes of this form apart from those of the form on another page.
    pub(super) id_salt: &'a str,
    pub(super) purpose: Purpose,
    pub(super) save: Save,
    /// Why the last save of this form was refused.
    pub(super) refusal: Option<&'a Failure>,
}

/// Draws the form and says which of its buttons was pressed. The page itself sends the save,
/// because a new media and an edit are sent and followed in different ways.
pub(super) fn media_form(
    ui: &mut egui::Ui,
    fields: &mut MediaFields,
    setup: FormSetup<'_>,
) -> Option<Pressed> {
    ui.push_id(setup.id_salt, |ui| {
        let is_sent = setup.save == Save::Sent;
        ui.add_enabled_ui(!is_sent, |ui| boxes(ui, fields, setup.purpose));
        ui.add_space(space::MD);
        let pressed = buttons(ui, setup.purpose, setup.save);
        if let Some(failure) = setup.refusal {
            ui.add_space(space::SM);
            Notice::error(&failure.hint).show(ui);
        }
        pressed
    })
    .inner
}

fn boxes(ui: &mut egui::Ui, fields: &mut MediaFields, purpose: Purpose) {
    caption(ui, CATEGORY);
    ui.horizontal(|ui| {
        for category in Category::ALL {
            let chip = Chip::plain(category.label()).selected(category == fields.category);
            if ui.add(chip).clicked() {
                fields.category = category;
            }
        }
    });
    if purpose == Purpose::New {
        caption(ui, MEDIA_TITLE);
        TextInput::new(MEDIA_TITLE, &mut fields.title)
            .placeholder("The new media's title")
            .show(ui);
    }
    caption(ui, AUTHORS);
    TextInput::new(AUTHORS, &mut fields.authors)
        .placeholder(LIST_PLACEHOLDER)
        .show(ui);
    caption(ui, TAGS);
    TextInput::new(TAGS, &mut fields.tags)
        .placeholder(LIST_PLACEHOLDER)
        .show(ui);
}

fn buttons(ui: &mut egui::Ui, purpose: Purpose, save: Save) -> Option<Pressed> {
    let note = match purpose {
        Purpose::New => NEW_NOTE,
        Purpose::Edit => EDIT_NOTE,
    };
    ui.horizontal(|ui| {
        let is_sent = save == Save::Sent;
        let save_button = Button::secondary(SAVE_MEDIA).loading(is_sent);
        let is_save_pressed = ui.add_enabled(save == Save::On, save_button).clicked();
        let is_cancel_pressed = ui
            .add_enabled(!is_sent, Button::secondary(CANCEL))
            .clicked();
        ui.add(egui::Label::new(TextRole::Small.rich(note)).truncate());
        if is_save_pressed {
            Some(Pressed::Save)
        } else if is_cancel_pressed {
            Some(Pressed::Cancel)
        } else {
            None
        }
    })
    .inner
}
