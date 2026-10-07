//! The one line beside the filters: why the bar cannot ask, or what is wrong with the library,
//! and the button that fixes it.

use eframe::egui;

use super::layout::slot;
use crate::contract::{AskDraft, Failure, Intent, Loadable};
use crate::state::Shared;
use crate::theme::{TextRole, Tone, color};
use crate::widgets::{self, Button, ControlSize};

const BLANK: &str = "Type a question to ask.";
const LIBRARY_LOADING: &str = "Loading the library\u{2026}";
const LIBRARY_EMPTY: &str = "Your library is empty. Add a chapter with rag-ingest.";
const LIBRARY_FAILED: &str = "The library did not load. ";
const TRY_AGAIN: &str = "Try again";
const SPINNER_LABEL: &str = "Library is loading";

/// Why the bar cannot ask, or `None` when it can.
pub(super) fn refusal(draft: &AskDraft) -> Option<&'static str> {
    draft.question.trim().is_empty().then_some(BLANK)
}

/// What the line says. The first case that holds wins.
#[derive(Debug, Clone, Copy)]
pub(super) enum Note<'a> {
    LibraryFailed(&'a Failure),
    LibraryLoading,
    LibraryEmpty,
    Blank,
}

impl<'a> Note<'a> {
    pub(super) fn of(shared: &'a Shared, draft: &AskDraft) -> Option<Note<'a>> {
        match &shared.library.catalogue {
            Loadable::Failed(failure) => return Some(Note::LibraryFailed(failure)),
            Loadable::Loading => return Some(Note::LibraryLoading),
            Loadable::Ready(catalogue) if catalogue.documents().next().is_none() => {
                return Some(Note::LibraryEmpty);
            }
            Loadable::Idle | Loadable::Ready(_) => {}
        }
        let is_refused = refusal(draft).is_some();
        (is_refused && !shared.ask.is_running()).then_some(Note::Blank)
    }

    pub(super) fn show(self, ui: &mut egui::Ui, rect: egui::Rect, intents: &mut Vec<Intent>) {
        let (text, colour) = self.words();
        slot(ui, "note", rect, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // The line is laid out right to left, so what comes after the text is drawn first.
                match self {
                    Note::LibraryFailed(_) => {
                        let retry = Button::secondary(TRY_AGAIN).size(ControlSize::Small);
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
