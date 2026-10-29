//! The written answer: its title, headings, paragraphs and the cards they point to.

use eframe::egui;

use super::pane::Pane;
use super::phase::{AnswerTab, Written};
use super::{cards, results, states};
use crate::media::rich_text;
use crate::panels::heights::Heights;
use crate::theme::{TextRole, space};
use crate::widgets::Copyable;
use contract::{Answer, AnswerBlock, Intent, ItemKind};

/// What a block of the answer is taken to be high until it has been drawn once.
const BLOCK_ESTIMATE: f32 = 2.0 * TextRole::Body.line_height();
/// The widest the written answer runs, in points: about seventy letters of body text. A pane
/// that is narrower uses its whole width.
const MEASURE: f32 = 720.0;
const COPY_ANSWER: &str = "Copy answer";

/// Narrows `ui` to the reading column. `set_max_width` alone would also widen a pane that is
/// narrower than the column, so the narrower of the two is taken.
// SMELL: the cards call this while this file calls the cards, so the two files depend on each
// other. The reading column wants a home that both can use.
pub(super) fn narrow_to_measure(ui: &mut egui::Ui) {
    ui.set_max_width(ui.available_width().min(MEASURE));
}

pub(super) fn show(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, written: Written<'_>) {
    let Written::Ready(answer) = written else {
        ui.add_space(space::MD);
        states::line(ui, written);
        results::list(ui, pane, AnswerTab::Results);
        return;
    };
    egui::ScrollArea::vertical()
        .id_salt(("answer", pane.ask.generation))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = space::MD;
            ui.add_space(space::MD);
            blocks(ui, pane, answer);
            ui.add_space(space::LG);
        });
}

fn blocks(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, answer: &Answer) {
    let ask = pane.ask;
    let title = answer.title.as_deref().unwrap_or(&ask.question);
    let mut heights = Heights::take(&mut pane.view.block_heights, BLOCK_ESTIMATE);
    heights.show(ui, 0, |ui| {
        narrow_to_measure(ui);
        let heading = Copyable::new(COPY_ANSWER).show(ui, |ui| {
            ui.label(TextRole::Title.rich(ui, title));
        });
        if heading.copied {
            pane.cx.intents.push(Intent::ShareAnswer);
        }
    });
    for (place, block) in answer.blocks.iter().enumerate() {
        let index = place + 1;
        match block {
            AnswerBlock::Heading(text) => {
                heights.show(ui, index, |ui| {
                    narrow_to_measure(ui);
                    ui.add_space(space::SM);
                    ui.label(TextRole::Heading.rich(ui, text));
                });
            }
            AnswerBlock::Paragraph { text, cites } => {
                heights.show(ui, index, |ui| {
                    narrow_to_measure(ui);
                    let paragraph = rich_text::RichText::new(text, TextRole::Body)
                        .cites(cites)
                        .selected(ask.selected_result);
                    pane.rich(ui, &paragraph);
                });
            }
            // A card for a result that is not there, or for a passage, draws nothing and takes
            // no room.
            AnswerBlock::Item(number) => {
                let Some((item, row)) = pane.found(*number) else {
                    continue;
                };
                let card = match item.kind {
                    ItemKind::Formula => cards::formula,
                    ItemKind::Figure => cards::figure,
                    ItemKind::Table => cards::table,
                    ItemKind::Code => cards::code,
                    ItemKind::Chunk => continue,
                };
                heights.show(ui, index, |ui| card(ui, pane, item, row));
            }
        }
    }
    pane.view.block_heights = heights;
}
