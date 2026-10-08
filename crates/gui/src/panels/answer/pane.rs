//! What every part of the pane draws from: the ask, the phase it is in, the words of the rows
//! and the pane's own small state, with the few things a click can do.

use eframe::egui;

use super::heights::Heights;
use super::listing::{Listing, Row};
use super::phase::{AnswerTab, Phase};
use crate::contract::{Intent, ResultItem};
use crate::media::rich_text::{self, Clicked};
use crate::panels::PanelCx;
use crate::state::AskSession;
use crate::theme::{TextRole, size, space};

/// What a block of the answer is taken to be high until it has been drawn once.
const BLOCK_ESTIMATE: f32 = 2.0 * TextRole::Body.line_height();

// SMELL: this repeats the layout of a result row, which the list of results owns. It is here
// because the list draws from the pane, and the two must not import each other.
/// What a result row is taken to be high until it has been drawn once: a card with the chip
/// line, the reason and three lines of preview.
const ROW_ESTIMATE: f32 =
    2.0 * space::MD + size::CHIP + 2.0 * space::SM + 4.0 * TextRole::Small.line_height();

/// The button that reads "Copied". It goes back to its name when the pointer leaves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Copied {
    Answer,
    Latex(usize),
}

#[derive(Debug)]
pub(super) struct View {
    /// `None`: the tab the mode of the ask opens.
    pub(super) tab: Option<AnswerTab>,
    /// The selection that the open list has already brought into view.
    pub(super) revealed: Option<usize>,
    pub(super) copied: Option<Copied>,
    pub(super) block_heights: Heights,
    pub(super) row_heights: Heights,
}

impl Default for View {
    fn default() -> View {
        View {
            tab: None,
            revealed: None,
            copied: None,
            block_heights: Heights::estimating(BLOCK_ESTIMATE),
            row_heights: Heights::estimating(ROW_ESTIMATE),
        }
    }
}

pub(super) struct Pane<'a, 'c> {
    pub(super) ask: &'a AskSession,
    pub(super) phase: Phase<'a>,
    /// `None` until the search has come back. A search that found nothing has a listing too,
    /// with every count at zero.
    pub(super) listing: Option<&'a Listing>,
    pub(super) view: &'a mut View,
    pub(super) cx: &'a mut PanelCx<'c>,
}

impl<'a> Pane<'a, '_> {
    pub(super) fn active(&self) -> AnswerTab {
        self.view.tab.unwrap_or(AnswerTab::opened_by(self.ask.mode))
    }

    pub(super) fn is_selected(&self, number: usize) -> bool {
        self.ask.selected_result == Some(number)
    }

    pub(super) fn found(&self, number: usize) -> Option<(&'a ResultItem, &'a Row)> {
        let results = self.ask.results();
        let at = results.iter().position(|item| item.number == number)?;
        Some((&results[at], self.listing?.rows.get(at)?))
    }

    pub(super) fn select(&mut self, number: usize) {
        self.cx.intents.push(Intent::SelectResult(number));
    }

    pub(super) fn copy(&mut self, text: &str, mark: Copied) {
        self.cx.intents.push(Intent::CopyText(text.to_owned()));
        self.view.copied = Some(mark);
    }

    pub(super) fn share(&mut self) {
        self.cx.intents.push(Intent::ShareAnswer);
        self.view.copied = Some(Copied::Answer);
    }

    pub(super) fn rich(&mut self, ui: &mut egui::Ui, text: &rich_text::RichText<'_>) {
        let clicked = rich_text::show(ui, self.cx.media, text);
        self.route(clicked);
    }

    pub(super) fn table(&mut self, ui: &mut egui::Ui, markdown: &str, role: TextRole) {
        let clicked = rich_text::table(ui, self.cx.media, markdown, role);
        self.route(clicked);
    }

    fn route(&mut self, clicked: Option<Clicked>) {
        match clicked {
            Some(Clicked::Citation(number)) => self.select(number),
            Some(Clicked::CopyText(source)) => self.cx.intents.push(Intent::CopyText(source)),
            None => {}
        }
    }
}
