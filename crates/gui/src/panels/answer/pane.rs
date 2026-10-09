//! What every part of the pane draws from: the ask, the phase it is in, the words of the rows
//! and the pane's own small state, with the few things a click can do.

use eframe::egui;

use super::heights::Heights;
use super::listing::{Listing, Row};
use super::phase::{AnswerTab, Phase};
use crate::contract::{AskMode, Intent, Loadable, ResultItem, Usage};
use crate::media::rich_text::{self, Clicked};
use crate::panels::PanelCx;
use crate::state::AskSession;
use crate::theme::TextRole;

/// The button that reads "Copied". It goes back to its name when the pointer leaves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Copied {
    Answer,
    Latex(usize),
}

#[derive(Debug, Default)]
pub(super) struct View {
    /// `None`: the tab the mode of the ask opens.
    pub(super) tab: Option<AnswerTab>,
    /// The selection that the open list has already brought into view.
    pub(super) revealed: Option<usize>,
    pub(super) copied: Option<Copied>,
    pub(super) block_heights: Heights,
    pub(super) row_heights: Heights,
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

    /// What the ask used, once there is something to tell: the whole ask when its answer has
    /// landed, and the search alone when the ask is for its results alone and found some.
    pub(super) fn spent(&self) -> Option<&'a Usage> {
        if let Loadable::Ready(answer) = &self.ask.answer {
            return Some(&answer.usage);
        }
        match &self.ask.search {
            Loadable::Ready(reply)
                if self.ask.mode == AskMode::ResultsOnly && !reply.results.is_empty() =>
            {
                Some(&reply.usage)
            }
            _ => None,
        }
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
