//! The written answer: its title, headings, paragraphs and the cards they point to.

use eframe::egui;

use super::pane::Pane;
use super::phase::{AnswerTab, Written};
use super::{cards, results, states};
use crate::contract::{Answer, AnswerBlock, ItemKind};
use crate::media::rich_text;
use crate::theme::{TextRole, space};

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
            ui.add_space(space::MD);
            blocks(ui, pane, answer);
            ui.add_space(space::LG);
        });
}

fn blocks(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>, answer: &Answer) {
    let ask = pane.ask;
    let title = answer.title.as_deref().unwrap_or(&ask.question);
    let mut heights = std::mem::take(&mut pane.view.block_heights);
    heights.show(ui, 0, |ui| {
        ui.label(TextRole::Title.rich(title));
    });
    for (place, block) in answer.blocks.iter().enumerate() {
        let index = place + 1;
        match block {
            AnswerBlock::Heading(text) => {
                heights.show(ui, index, |ui| {
                    ui.add_space(space::SM);
                    ui.label(TextRole::Heading.rich(text));
                });
            }
            AnswerBlock::Paragraph { text, cites } => {
                heights.show(ui, index, |ui| {
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
                    ItemKind::Chunk => continue,
                };
                heights.show(ui, index, |ui| card(ui, pane, item, row));
            }
        }
    }
    pane.view.block_heights = heights;
}
