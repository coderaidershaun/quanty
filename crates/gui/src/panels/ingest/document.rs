//! The PDF section: the file, the name of the document as the category of its media asks for it,
//! and its own tags. What is typed here is held back until no box of the section has the
//! keyboard, so a check never starts on a keystroke.

use std::path::Path;

use eframe::egui;

use super::{Local, Missing};
use crate::contract::{Category, DocumentName, IngestRequest, Intent};
use crate::panels::labels::{caption, list_of, text_of};
use crate::theme::{TextRole, color, space};
use crate::widgets::{Button, TextInput, TextInputResponse};

const PDF: &str = "PDF";
const CHAPTER_NUMBER: &str = "Chapter number";
const CHAPTER_NAME: &str = "Chapter name";
const TITLE: &str = "Title";
const OWN_TAGS: &str = "Tags for this PDF";
const NUMBER_WIDTH: f32 = 120.0;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct DocumentFields {
    chapter_number: String,
    chapter_name: String,
    /// The title of a paper's or another media's PDF.
    pub(super) title: String,
    own_tags: String,
}

impl DocumentFields {
    pub(super) fn of(request: &IngestRequest) -> DocumentFields {
        let mut fields = DocumentFields {
            own_tags: text_of(&request.tags),
            ..DocumentFields::default()
        };
        match &request.name {
            DocumentName::Chapter { number, name } => {
                fields.chapter_number = number.to_string();
                fields.chapter_name.clone_from(name);
            }
            DocumentName::Title(title) => fields.title.clone_from(title),
        }
        fields
    }

    /// A book's PDF is a chapter, with a number and a name; any other PDF has a title.
    pub(super) fn name(&self, category: Category) -> Result<DocumentName, Missing> {
        match category {
            Category::Book => {
                let number = self.chapter_number.trim().parse::<u32>().ok();
                let name = self.chapter_name.trim();
                match number {
                    Some(number) if !name.is_empty() => Ok(DocumentName::Chapter {
                        number,
                        name: name.to_owned(),
                    }),
                    _ => Err(Missing::Chapter),
                }
            }
            Category::Paper | Category::Other => {
                let title = self.title.trim();
                if title.is_empty() {
                    Err(Missing::Title)
                } else {
                    Ok(DocumentName::Title(title.to_owned()))
                }
            }
        }
    }

    pub(super) fn tags(&self) -> Vec<String> {
        list_of(&self.own_tags)
    }

    /// The chapter fields belonged to the file before, so a file whose name is not a chapter's
    /// empties them.
    pub(super) fn take_chapter_of(&mut self, pdf: &Path) {
        let file_name = file_name_of(pdf).unwrap_or_default();
        match DocumentName::from_chapter_file_name(&file_name) {
            Some(DocumentName::Chapter { number, name }) => {
                self.chapter_number = number.to_string();
                self.chapter_name = name;
            }
            _ => {
                self.chapter_number.clear();
                self.chapter_name.clear();
            }
        }
    }
}

fn file_name_of(pdf: &Path) -> Option<String> {
    pdf.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

pub(super) fn show(ui: &mut egui::Ui, local: &mut Local, intents: &mut Vec<Intent>) {
    caption(ui, PDF);
    file_row(ui, local.pdf.as_deref(), intents);
    let category = local.media.chosen().map(|(_, category)| category);
    let typed = &mut local.typed;
    let mut boxes = Vec::new();
    match category {
        Some(Category::Book) => {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(NUMBER_WIDTH);
                    caption(ui, CHAPTER_NUMBER);
                    let number = TextInput::new(CHAPTER_NUMBER, &mut typed.chapter_number)
                        .id_salt("ingest_chapter_number")
                        .placeholder("For example 3")
                        .width(NUMBER_WIDTH)
                        .show(ui);
                    boxes.push(number);
                });
                ui.vertical(|ui| {
                    caption(ui, CHAPTER_NAME);
                    let name = TextInput::new(CHAPTER_NAME, &mut typed.chapter_name)
                        .id_salt("ingest_chapter_name")
                        .placeholder("For example Greeks")
                        .show(ui);
                    boxes.push(name);
                });
            });
        }
        Some(Category::Paper | Category::Other) => {
            caption(ui, TITLE);
            let title = TextInput::new(TITLE, &mut typed.title)
                .id_salt("ingest_document_title")
                .placeholder("The title of this PDF")
                .show(ui);
            boxes.push(title);
        }
        None => {}
    }
    caption(ui, OWN_TAGS);
    let tags = TextInput::new(OWN_TAGS, &mut typed.own_tags)
        .id_salt("ingest_own_tags")
        .placeholder("Optional. The media's tags apply as well.")
        .show(ui);
    boxes.push(tags);
    // Enter, Escape and a click elsewhere each take the keyboard from a box, so each of them
    // settles what was typed.
    if !boxes
        .iter()
        .any(|shown: &TextInputResponse| shown.response.has_focus())
    {
        local.settled = local.typed.clone();
    }
}

fn file_row(ui: &mut egui::Ui, pdf: Option<&Path>, intents: &mut Vec<Intent>) {
    ui.horizontal(|ui| {
        if ui.add(Button::secondary("Choose a PDF")).clicked() {
            intents.push(Intent::PickPdf);
        }
        let (words, tint) = match pdf.and_then(file_name_of) {
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
    ui.add_space(space::XS);
}
