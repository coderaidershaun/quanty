//! What follows the form: the check, the running ingest and the way it ended, each with the
//! buttons that go on from there.

use eframe::egui;

use super::Local;
use crate::contract::{
    ChapterLabel, ChapterState, Failure, IngestOutcome, IngestProgress, IngestReport,
    IngestRequest, IngestStage, Intent, ItemCounts, Preflight,
};
use crate::panels::PanelCx;
use crate::state::IngestJob;
use crate::theme::{TextRole, space};
use crate::widgets::{self, Button, Notice};

/// `draft` is the request the form holds now.
pub(super) fn show(
    ui: &mut egui::Ui,
    local: &mut Local,
    cx: &mut PanelCx<'_>,
    draft: Option<&IngestRequest>,
) {
    match &cx.shared.ingest {
        IngestJob::Idle => check_button(ui, draft, cx.intents),
        IngestJob::Checking { .. } => {
            ui.horizontal(|ui| {
                widgets::spinner(ui, "Checking the chapter");
                ui.label(TextRole::BodyStrong.rich("Checking the chapter"));
            });
        }
        IngestJob::Checked { preflight, .. } if preflight.blockers.is_empty() => {
            chapter_lines(ui, preflight);
            ui.add_space(space::MD);
            if ui.add(Button::primary("Start ingest")).clicked() {
                cx.intents.push(Intent::StartIngest);
            }
        }
        IngestJob::Checked { preflight, .. } => {
            if !preflight.chapter.name.is_empty() {
                chapter_title(ui, &preflight.chapter);
            }
            for blocker in &preflight.blockers {
                Notice::error(&blocker.hint).show(ui);
                detail(ui, blocker);
            }
            ui.add_space(space::MD);
            check_button(ui, draft, cx.intents);
        }
        IngestJob::CheckFailed { failure, .. } => {
            Notice::error("The chapter cannot be ingested yet")
                .body(&failure.hint)
                .show(ui);
            detail(ui, failure);
            ui.add_space(space::MD);
            check_button(ui, draft, cx.intents);
        }
        IngestJob::Running { progress, .. } | IngestJob::Stopping { progress, .. } => {
            running(ui, progress.as_ref());
        }
        IngestJob::Finished { request, result } => {
            finished(ui, local, cx.intents, request, result);
        }
    }
}

fn check_button(ui: &mut egui::Ui, draft: Option<&IngestRequest>, intents: &mut Vec<Intent>) {
    let button = Button::primary("Check the chapter");
    if ui.add_enabled(draft.is_some(), button).clicked()
        && let Some(request) = draft
    {
        intents.push(Intent::CheckIngest(request.clone()));
    }
}

fn detail(ui: &mut egui::Ui, failure: &Failure) {
    ui.label(TextRole::Small.rich(&failure.detail));
}

fn chapter_title(ui: &mut egui::Ui, chapter: &ChapterLabel) {
    ui.label(TextRole::BodyStrong.rich(format!("Chapter {} · {}", chapter.number, chapter.name)));
}

fn chapter_lines(ui: &mut egui::Ui, preflight: &Preflight) {
    chapter_title(ui, &preflight.chapter);
    let pages = preflight.pages;
    let line = match preflight.state {
        None => return,
        Some(ChapterState::New) => "Not converted yet.".to_owned(),
        Some(ChapterState::PartlyConverted { pages_done }) => match pages {
            Some(pages) => format!(
                "{pages_done} of {pages} pages are converted already and are not paid for again."
            ),
            None => {
                format!("{pages_done} pages are converted already and are not paid for again.")
            }
        },
        Some(ChapterState::Converted) => {
            let all = pages.map_or_else(
                || "All pages".to_owned(),
                |pages| format!("All {pages} pages"),
            );
            format!(
                "{all} are converted already: only the embedding and the concepts are paid for."
            )
        }
        Some(ChapterState::Ingested { items }) => format!(
            "Already ingested with {items} items: a start converts nothing and pays for nothing."
        ),
    };
    ui.label(TextRole::Body.rich(line));
}

fn running(ui: &mut egui::Ui, progress: Option<&IngestProgress>) {
    let words = match progress.map(|progress| progress.stage) {
        None => "Starting: reading the PDF and asking the stores",
        Some(IngestStage::PreparingPages | IngestStage::Converting) => "Converting the pages",
        Some(_) => "Storing the items and reading the concepts",
    };
    ui.horizontal(|ui| {
        widgets::spinner(ui, words);
        ui.label(TextRole::BodyStrong.rich(words));
    });
    ui.label(TextRole::Small.rich(
        "Keep the app open until this is done. If it stops, start the same PDF again: pages that are converted are not paid for twice.",
    ));
}

fn finished(
    ui: &mut egui::Ui,
    local: &mut Local,
    intents: &mut Vec<Intent>,
    request: &IngestRequest,
    result: &Result<IngestOutcome, Failure>,
) {
    match result {
        Ok(IngestOutcome::Ingested(report)) => {
            Notice::success(&format!("Ingested {}", report.title))
                .body(&summary(report))
                .show(ui);
        }
        Ok(IngestOutcome::AlreadyIngested { items, .. }) => {
            Notice::info("Already ingested")
                .body(&format!(
                    "{items} items are stored. Nothing was converted or embedded."
                ))
                .show(ui);
        }
        Ok(IngestOutcome::Cancelled) => {
            Notice::info("Stopped")
                .body("Start the same PDF again to go on.")
                .show(ui);
        }
        Err(failure) => {
            Notice::error("The ingest failed")
                .body(&failure.hint)
                .show(ui);
            detail(ui, failure);
            ui.label(
                TextRole::Small
                    .rich("Pages that are converted are kept and are not paid for twice."),
            );
        }
    }
    ui.add_space(space::MD);
    ui.horizontal(|ui| {
        if result.is_err() && ui.add(Button::primary("Try again")).clicked() {
            intents.push(Intent::CheckIngest(request.clone()));
        }
        if ui.add(Button::secondary("Add another chapter")).clicked() {
            intents.push(Intent::ClearIngest);
            local.pdf = None;
        }
    });
}

fn summary(report: &IngestReport) -> String {
    let mut text = format!(
        "{} pages, {}. {} concepts created, {} linked.",
        report.pages,
        counts(&report.items),
        report.concepts_created,
        report.concepts_linked
    );
    if let Some(cost) = report.cost_usd {
        text.push_str(&format!(" The conversion cost ${cost:.2}."));
    }
    match report.pages_to_check.len() {
        0 => {}
        1 => text.push_str(" 1 page needs a check."),
        pages => text.push_str(&format!(" {pages} pages need a check.")),
    }
    match report.skipped_items {
        0 => {}
        1 => text.push_str(" 1 item was skipped."),
        items => text.push_str(&format!(" {items} items were skipped.")),
    }
    text
}

/// The four counts in the words the notice of the app uses for them.
fn counts(items: &ItemCounts) -> String {
    fn count(number: u64, noun: &str) -> String {
        match number {
            1 => format!("1 {noun}"),
            _ => format!("{number} {noun}s"),
        }
    }
    format!(
        "{}, {}, {} and {}",
        count(items.chunks, "passage"),
        count(items.formulas, "formula"),
        count(items.figures, "figure"),
        count(items.tables, "table")
    )
}
