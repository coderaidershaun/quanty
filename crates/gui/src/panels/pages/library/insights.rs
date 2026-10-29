//! The Insights, Takeaways and Cheat sheet views of a document: the part of what was saved for it
//! that the view names, with the page each insight rests on and the export of all three, or what
//! is said while there are none.

use eframe::egui;

use super::width;
use crate::media::math::{self, MathRef};
use crate::media::rich_text::{self, Clicked, DollarBlock, RichText, dollar_blocks};
use crate::panels::exporting;
use crate::panels::labels::{COPY_COMMAND, TRY_AGAIN};
use crate::panels::{PanelCx, PanelWidth};
use crate::theme::{Icon, TextRole, space};
use crate::widgets::{Button, Chip, ControlSize, Placeholder};
use contract::{
    Catalogue, ChapterInsights, DocId, DocumentView, ExportPlace, Failure, Insight, Intent,
    Loadable,
};

const NO_INSIGHTS_FOR_PART: &str = "No insights for this part";
const NO_INSIGHTS_FOR_PART_HINT: &str =
    "A part of a book that is not a chapter, such as its index or its contents, gets no insights.";
const READING: &str = "Reading the insights";
const UNREADABLE: &str = "The insights could not be read";
const NOT_WRITTEN: &str = "No insights yet";
const EXPORT: &str = "Export";
const NO_INSIGHTS: &str = "No insights";
const NO_TAKEAWAYS: &str = "No takeaways";
const NO_CHEAT_SHEET: &str = "No cheat sheet";

/// `part` is the view that is shown: `Insights`, `Takeaways` or `CheatSheet`.
pub(super) fn show(ui: &mut egui::Ui, part: DocumentView, cx: &mut PanelCx<'_>) {
    let shared = cx.shared;
    let Some(doc) = shared.shelf.document() else {
        return;
    };
    let Some(catalogue) = shared.library.catalogue.ready() else {
        return;
    };
    // The rule of the part comes first, so what a part says does not depend on its folder or on
    // whatever the slot holds.
    if !catalogue.gets_insights(doc) {
        Placeholder::empty(Icon::INSIGHT, NO_INSIGHTS_FOR_PART)
            .hint(NO_INSIGHTS_FOR_PART_HINT)
            .show(ui);
        return;
    }
    let slot = &shared.insights;
    match &slot.saved {
        Loadable::Ready(Some(saved)) if slot.doc == Some(doc) => {
            saved_part(ui, cx, catalogue, doc, saved, part);
        }
        Loadable::Ready(None) if slot.doc == Some(doc) => not_written(ui, cx, doc),
        Loadable::Failed(failure) if slot.doc == Some(doc) => failed(ui, cx, failure, part),
        _ => {
            Placeholder::loading(READING).show(ui);
        }
    }
}

fn not_written(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, doc: DocId) {
    let command = ChapterInsights::command_for(doc);
    let hint = format!(
        "They are written when a chapter is ingested. To write them for this one, run {command}. It asks the model of the Ingest choice once, which is paid."
    );
    let placeholder = Placeholder::empty(Icon::INSIGHT, NOT_WRITTEN)
        .hint(&hint)
        .action(COPY_COMMAND)
        .action_size(width(ui).control_size())
        .show(ui);
    if placeholder.action_clicked {
        cx.intents.push(Intent::CopyText(command));
    }
}

fn failed(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, failure: &Failure, part: DocumentView) {
    let placeholder = Placeholder::error(UNREADABLE)
        .hint(&failure.hint)
        .action(TRY_AGAIN)
        .action_size(width(ui).control_size())
        .show(ui);
    if placeholder.action_clicked {
        cx.intents.push(Intent::ShowDocumentView(part));
    }
}

fn saved_part(
    ui: &mut egui::Ui,
    cx: &mut PanelCx<'_>,
    catalogue: &Catalogue,
    doc: DocId,
    saved: &ChapterInsights,
    part: DocumentView,
) {
    let written_by = TextRole::Small.rich(ui, format!("Written by {}", saved.model));
    let panel = width(ui);
    let size = panel.control_size();
    if panel == PanelWidth::Narrow {
        // The label may wrap, so the button gets a line of its own, tall enough for a finger.
        ui.label(written_by);
        export_menu(ui, cx, catalogue, size);
    } else {
        ui.horizontal(|ui| {
            ui.label(written_by);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                export_menu(ui, cx, catalogue, size);
            });
        });
    }
    exporting::strip(ui, cx, ExportPlace::Insights { doc });
    ui.add_space(space::SM);
    egui::ScrollArea::vertical()
        .id_salt(("insights", doc, part))
        .auto_shrink([false, true])
        .show(ui, |ui| match part {
            DocumentView::Takeaways => takeaways(ui, cx, &saved.takeaways),
            DocumentView::CheatSheet => cheat_sheet(ui, cx, &saved.cheat_sheet),
            _ => insights(ui, cx, doc, &saved.insights),
        });
}

fn insights(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, doc: DocId, insights: &[Insight]) {
    if insights.is_empty() {
        Placeholder::empty(Icon::INSIGHT, NO_INSIGHTS).show(ui);
        return;
    }
    for insight in insights {
        insight_paragraph(ui, cx, doc, insight);
    }
}

fn takeaways(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, takeaways: &[String]) {
    if takeaways.is_empty() {
        Placeholder::empty(Icon::INSIGHT, NO_TAKEAWAYS).show(ui);
        return;
    }
    let lines: Vec<String> = takeaways
        .iter()
        .map(|takeaway| format!("- {takeaway}"))
        .collect();
    blocks(ui, cx, &dollar_blocks(&lines.join("\n")));
}

fn cheat_sheet(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, cheat_sheet: &str) {
    if cheat_sheet.trim().is_empty() {
        Placeholder::empty(Icon::INSIGHT, NO_CHEAT_SHEET).show(ui);
        return;
    }
    blocks(ui, cx, &dollar_blocks(cheat_sheet));
}

fn export_menu(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, catalogue: &Catalogue, size: ControlSize) {
    let shared = cx.shared;
    let button = Button::secondary(EXPORT).icon(Icon::DOCUMENT).size(size);
    let response = ui.add(button);
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            let request = |format| shared.insights.export_request(catalogue, format);
            if exporting::items(ui, cx, size, request) {
                ui.close();
            }
        });
}

/// One insight, then a chip for each page it rests on.
fn insight_paragraph(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, doc: DocId, insight: &Insight) {
    blocks(ui, cx, &dollar_blocks(&insight.text));
    ui.add_space(space::XS);
    let chip_height = width(ui).chip_height();
    ui.horizontal_wrapped(|ui| {
        for page in &insight.pages {
            let chip = ui.add(Chip::plain(&format!("Page {page}")).min_height(chip_height));
            let hover = format!("Open page {page} in the Source panel");
            if chip.on_hover_text(hover).clicked() {
                cx.intents.push(Intent::OpenSource {
                    doc,
                    page: *page,
                    piece: None,
                });
            }
        }
    });
    ui.add_space(space::SM);
}

fn blocks(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, blocks: &[DollarBlock]) {
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            ui.add_space(space::SM);
        }
        let clicked = match block {
            DollarBlock::Heading(words) => {
                rich_text::show(ui, cx.media, &RichText::new(words, TextRole::Subheading))
            }
            DollarBlock::Text(text) => {
                rich_text::show(ui, cx.media, &RichText::new(text, TextRole::Body))
            }
            DollarBlock::Formula(latex) => {
                math::show(ui, cx.media, &MathRef::block(latex));
                None
            }
            DollarBlock::Table(markdown) => {
                rich_text::table(ui, cx.media, markdown, TextRole::Body)
            }
        };
        // Cites are never given to this text, so a citation cannot be clicked.
        if let Some(Clicked::CopyText(text)) = clicked {
            cx.intents.push(Intent::CopyText(text));
        }
    }
}
