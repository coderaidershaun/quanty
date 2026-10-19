//! The PDF section: the file, the name of the document as the category of its media asks for it
//! (for a book, a chapter or a part that is not a chapter), and its own tags. What is typed here
//! is held back until no box of the section has the keyboard, so a check never starts on a
//! keystroke.

use std::path::Path;

use eframe::egui;

use super::{Local, Missing};
use crate::contract::{Category, DocumentName, IngestRequest, Intent};
use crate::panels::labels::{caption, list_of, text_of};
use crate::theme::{TextRole, color, space};
use crate::widgets::{Button, Chip, TextInput, TextInputResponse};

const PDF: &str = "PDF";
const PART_OF_THE_BOOK: &str = "Part of the book";
const CHAPTER: &str = "Chapter";
const OTHER_PART: &str = "Other part";
const CHAPTER_NUMBER: &str = "Chapter number";
const CHAPTER_NAME: &str = "Chapter name";
const TITLE: &str = "Title";
const OWN_TAGS: &str = "Tags for this PDF";
const NUMBER_WIDTH: f32 = 120.0;

/// What a book's PDF is: one of its chapters, or a part such as its index or its glossary.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum BookPart {
    #[default]
    Chapter,
    Other,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct DocumentFields {
    part: BookPart,
    chapter_number: String,
    chapter_name: String,
    /// The title of a paper's or another media's PDF.
    pub(super) title: String,
    /// The title of a part of a book that is not a chapter. It is kept apart from `title`, so
    /// naming a paper after its media never writes over it.
    part_title: String,
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
            DocumentName::Title(title) if request.category == Category::Book => {
                fields.part = BookPart::Other;
                fields.part_title.clone_from(title);
            }
            DocumentName::Title(title) => fields.title.clone_from(title),
        }
        fields
    }

    /// A book's PDF is a chapter, with a number and a name, or a part with a title; any other PDF
    /// has a title.
    pub(super) fn name(&self, category: Category) -> Result<DocumentName, Missing> {
        match (category, self.part) {
            (Category::Book, BookPart::Chapter) => {
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
            (Category::Book, BookPart::Other) => titled(&self.part_title),
            (Category::Paper | Category::Other, _) => titled(&self.title),
        }
    }

    pub(super) fn tags(&self) -> Vec<String> {
        list_of(&self.own_tags)
    }

    /// The name fields belonged to the file before. A file named as a chapter fills in the
    /// chapter. Any other file empties the chapter and offers its words as the title of a part,
    /// but leaves the choice of the part alone: the person decides that a file is not a chapter.
    pub(super) fn take_name_of(&mut self, pdf: &Path) {
        let file_name = file_name_of(pdf).unwrap_or_default();
        match DocumentName::from_chapter_file_name(&file_name) {
            Some(DocumentName::Chapter { number, name }) => {
                self.part = BookPart::Chapter;
                self.chapter_number = number.to_string();
                self.chapter_name = name;
                self.part_title.clear();
            }
            _ => {
                self.chapter_number.clear();
                self.chapter_name.clear();
                self.part_title = DocumentName::title_from_file_name(&file_name);
            }
        }
    }
}

fn titled(title: &str) -> Result<DocumentName, Missing> {
    let title = title.trim();
    if title.is_empty() {
        Err(Missing::Title)
    } else {
        Ok(DocumentName::Title(title.to_owned()))
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
    let mut boxes = name_boxes(ui, category, &mut local.typed);
    caption(ui, OWN_TAGS);
    let tags = TextInput::new(OWN_TAGS, &mut local.typed.own_tags)
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

fn name_boxes(
    ui: &mut egui::Ui,
    category: Option<Category>,
    typed: &mut DocumentFields,
) -> Vec<TextInputResponse> {
    let mut boxes = Vec::new();
    match category {
        Some(Category::Book) => {
            part_chips(ui, &mut typed.part);
            match typed.part {
                BookPart::Chapter => chapter_boxes(ui, typed, &mut boxes),
                BookPart::Other => {
                    caption(ui, TITLE);
                    let title = TextInput::new(TITLE, &mut typed.part_title)
                        .id_salt("ingest_part_title")
                        .placeholder("For example Index")
                        .show(ui);
                    boxes.push(title);
                }
            }
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
    boxes
}

fn chapter_boxes(
    ui: &mut egui::Ui,
    typed: &mut DocumentFields,
    boxes: &mut Vec<TextInputResponse>,
) {
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

fn part_chips(ui: &mut egui::Ui, part: &mut BookPart) {
    caption(ui, PART_OF_THE_BOOK);
    ui.horizontal(|ui| {
        for (choice, label) in [(BookPart::Chapter, CHAPTER), (BookPart::Other, OTHER_PART)] {
            let chip = Chip::plain(label).selected(choice == *part);
            if ui.add(chip).clicked() {
                *part = choice;
            }
        }
    });
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
