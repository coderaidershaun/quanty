//! The one line that says why the bar cannot ask, or what is wrong with the library, and the
//! button that fixes it. It stands beside the filters, or on a row of its own on a narrow bar,
//! where only a note about the library shows.

use eframe::egui;

use super::layout::slot;
use crate::panels::labels::TRY_AGAIN;
use crate::state::Shared;
use crate::theme::{TextRole, Tone, color};
use crate::widgets::{self, Button, ControlSize};
use contract::{AskDraft, Failure, Intent, Loadable};

const BLANK: &str = "Type a question to ask.";
const LIBRARY_LOADING: &str = "Loading the library\u{2026}";
const LIBRARY_EMPTY: &str = "Your library is empty. Add a PDF on the Ingest tab.";
const LIBRARY_FAILED: &str = "The library did not load. ";
const SPINNER_LABEL: &str = "Library is loading";

/// Whether a narrow bar has a row of its own for a note. Only a note about the library has one: the
/// note for an empty question would make the bar move while a person types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteRow {
    Shown,
    Absent,
}

/// The row that a narrow bar needs for the state of the library.
pub(crate) fn note_row(shared: &Shared) -> NoteRow {
    match Note::of_library(shared) {
        Some(_) => NoteRow::Shown,
        None => NoteRow::Absent,
    }
}

pub(super) fn refusal(draft: &AskDraft) -> Option<&'static str> {
    draft.question.trim().is_empty().then_some(BLANK)
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Note<'a> {
    LibraryFailed(&'a Failure),
    LibraryLoading,
    LibraryEmpty,
    Blank,
}

impl<'a> Note<'a> {
    pub(super) fn of(shared: &'a Shared, draft: &AskDraft) -> Option<Note<'a>> {
        if let Some(note) = Note::of_library(shared) {
            return Some(note);
        }
        let is_refused = refusal(draft).is_some();
        (is_refused && !shared.ask.is_running()).then_some(Note::Blank)
    }

    /// The note about the library, which shows whatever the box holds.
    pub(super) fn of_library(shared: &'a Shared) -> Option<Note<'a>> {
        match &shared.library.catalogue {
            Loadable::Failed(failure) => Some(Note::LibraryFailed(failure)),
            Loadable::Loading => Some(Note::LibraryLoading),
            Loadable::Ready(catalogue) if catalogue.documents().next().is_none() => {
                Some(Note::LibraryEmpty)
            }
            Loadable::Idle | Loadable::Ready(_) => None,
        }
    }

    pub(super) fn show(
        self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        size: ControlSize,
        intents: &mut Vec<Intent>,
    ) {
        let (text, colour) = self.words();
        slot(ui, "note", rect, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // The line is laid out right to left, so what comes after the text is drawn first.
                match self {
                    Note::LibraryFailed(_) => {
                        let retry = Button::secondary(TRY_AGAIN).size(size);
                        if ui.add(retry).clicked() {
                            intents.push(Intent::RefreshCatalogue);
                        }
                    }
                    Note::LibraryLoading => {
                        widgets::spinner(ui, SPINNER_LABEL);
                    }
                    Note::LibraryEmpty | Note::Blank => {}
                }
                let line =
                    ui.add(egui::Label::new(TextRole::Small.rich(text).color(colour)).truncate());
                if let Note::LibraryFailed(failure) = self {
                    line.on_hover_text(&failure.detail);
                }
            });
        });
    }

    fn words(self) -> (String, egui::Color32) {
        match self {
            Note::LibraryFailed(failure) => (
                format!("{LIBRARY_FAILED}{}", failure.hint),
                Tone::Warning.swatch().text,
            ),
            Note::LibraryLoading => (LIBRARY_LOADING.to_owned(), color::TEXT_MUTED),
            Note::LibraryEmpty => (LIBRARY_EMPTY.to_owned(), color::TEXT_SECONDARY),
            Note::Blank => (BLANK.to_owned(), color::TEXT_MUTED),
        }
    }
}
