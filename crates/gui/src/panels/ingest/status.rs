//! What follows the PDF: what the draft still lacks, the check, the running ingest and the way it
//! ended, each with the buttons that go on from there.

use eframe::egui;

use super::{Local, Missing};
use crate::contract::{
    ChapterState, Failure, IngestOutcome, IngestProgress, IngestReport, IngestRequest, IngestStage,
    Intent, Preflight,
};
use crate::panels::PanelCx;
use crate::panels::labels::{cost_text, spent_text, tokens_by_model};
use crate::state::IngestJob;
use crate::theme::{TextRole, color, space};
use crate::widgets::{self, Button, Notice};

const CHECKING: &str = "Checking the PDF";
const CONNECTING: &str = "Connecting to the stores";
const TRY_AGAIN: &str = "Try again";
const COST: &str = "Checking is free. Starting is paid work: claude and Jev convert each page, Gemini embeds the items, and claude reads the concepts.";
const KEEP_OPEN: &str = "Keep the app open until this is done. If it stops, start the same PDF again: pages that are converted are not paid for twice.";

/// `draft` is what the form holds now, made of its settled fields.
pub(super) fn show(
    ui: &mut egui::Ui,
    local: &mut Local,
    cx: &mut PanelCx<'_>,
    draft: &Result<IngestRequest, Missing>,
) {
    match &cx.shared.ingest {
        // A draft that is complete is checked in this frame, so it needs no words.
        IngestJob::Idle => {
            if let Err(missing) = draft {
                ui.label(TextRole::Small.rich(missing.hint()));
            }
        }
        IngestJob::Checking { .. } => {
            widgets::indeterminate_bar(ui, CHECKING);
            ui.label(TextRole::Small.rich(CHECKING));
        }
        IngestJob::Checked { request, preflight } if preflight.blockers.is_empty() => {
            checked_line(ui, preflight);
            ui.add_space(space::MD);
            // A box may still have the keyboard, so Start reads the boxes as typed. A typed change
            // that makes another request is not checked yet, and a start would run the old one.
            let can_start = local.draft_of(&local.typed).as_ref() == Ok(request);
            if ui
                .add_enabled(can_start, Button::primary("Start ingest"))
                .clicked()
            {
                cx.intents.push(Intent::StartIngest);
            }
            ui.add_space(space::SM);
            ui.label(TextRole::Small.rich(COST));
        }
        IngestJob::Checked { preflight, .. } => {
            ui.label(TextRole::BodyStrong.rich(preflight.name.label()));
            for blocker in &preflight.blockers {
                Notice::error(&blocker.hint).show(ui);
                detail(ui, blocker);
            }
            ui.add_space(space::MD);
            try_again(ui, draft, cx.intents);
        }
        IngestJob::CheckFailed { failure, .. } => {
            Notice::error("The PDF cannot be ingested yet")
                .body(&failure.hint)
                .show(ui);
            detail(ui, failure);
            ui.add_space(space::MD);
            try_again(ui, draft, cx.intents);
        }
        IngestJob::Running { progress, .. } | IngestJob::Stopping { progress, .. } => {
            running(ui, progress.as_ref());
        }
        IngestJob::Finished { request, result } => {
            finished(ui, local, cx.intents, request, result);
        }
    }
}

/// After a sign-in or a store that started, the draft is the same, so nothing else would check it
/// again.
fn try_again(ui: &mut egui::Ui, draft: &Result<IngestRequest, Missing>, intents: &mut Vec<Intent>) {
    let button = Button::primary(TRY_AGAIN);
    if ui.add_enabled(draft.is_ok(), button).clicked()
        && let Ok(request) = draft
    {
        intents.push(Intent::CheckIngest(request.clone()));
    }
}

fn detail(ui: &mut egui::Ui, failure: &Failure) {
    ui.label(TextRole::Small.rich(&failure.detail));
}

fn checked_line(ui: &mut egui::Ui, preflight: &Preflight) {
    let mut line = preflight.name.label();
    if let Some(pages) = preflight.pages {
        line.push_str(&format!(" — {pages} pages"));
    }
    let words = match preflight.state {
        None => None,
        Some(ChapterState::New) => Some("Not converted yet.".to_owned()),
        Some(ChapterState::PartlyConverted { pages_done }) => Some(format!(
            "{pages_done} are converted already and are not paid for again."
        )),
        Some(ChapterState::Converted) => Some(
            "All are converted already: only the embedding and the concepts are paid for."
                .to_owned(),
        ),
        Some(ChapterState::Ingested { items }) => Some(format!(
            "Already ingested with {items} items: a start converts nothing and pays for nothing."
        )),
    };
    if let Some(words) = words {
        line.push_str(". ");
        line.push_str(&words);
    }
    ui.label(TextRole::Body.rich(line));
}

fn stage_words(stage: Option<IngestStage>) -> String {
    let Some(stage) = stage else {
        return CONNECTING.to_owned();
    };
    let words = match stage {
        IngestStage::CheckingStored => "Checking what is already stored".to_owned(),
        IngestStage::OpeningPdf => "Opening the PDF".to_owned(),
        IngestStage::PreparingPages { ready, pages, .. } => {
            format!("Preparing the pages — {ready} of {pages} ready")
        }
        IngestStage::Converting { saved, pages } => {
            format!("Converting the pages — {saved} of {pages} done")
        }
        IngestStage::WritingGraph => "Writing the graph".to_owned(),
        IngestStage::Embedding { items } => format!("Embedding {items} items"),
        IngestStage::Storing => "Storing the items".to_owned(),
        IngestStage::ReadingConcepts { done, items } => {
            format!("Reading the concepts — {done} of {items}")
        }
        IngestStage::LinkingConcepts { done, items } => {
            format!("Linking the concepts — {done} of {items}")
        }
    };
    match stage.share_done() {
        Some(share) => format!("{words} · about {}%", (share * 100.0).round()),
        None => words,
    }
}

fn running(ui: &mut egui::Ui, progress: Option<&IngestProgress>) {
    let stage = progress.map(|progress| progress.stage);
    let words = stage_words(stage);
    ui.horizontal(|ui| {
        ui.label(TextRole::BodyStrong.rich(&words));
        if let Some(progress) = progress {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                spent_so_far(ui, progress);
            });
        }
    });
    ui.add_space(space::XS);
    match stage.and_then(IngestStage::share_done) {
        Some(share) => widgets::progress_bar(ui, &words, share),
        None => widgets::indeterminate_bar(ui, &words),
    };
    ui.add_space(space::SM);
    ui.label(TextRole::Small.rich(KEEP_OPEN));
}

fn spent_so_far(ui: &mut egui::Ui, progress: &IngestProgress) {
    if progress.spent.tokens() > 0 {
        let spent = format!("{} so far", spent_text(&progress.spent));
        ui.label(TextRole::Small.rich(spent).color(color::TEXT_MUTED));
    }
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
        if result.is_err() && ui.add(Button::primary(TRY_AGAIN)).clicked() {
            intents.push(Intent::CheckIngest(request.clone()));
        }
        if ui.add(Button::secondary("Add another PDF")).clicked() {
            intents.push(Intent::ClearIngest);
            local.add_another_pdf();
        }
    });
}

fn summary(report: &IngestReport) -> String {
    let mut text = format!(
        "{} pages, {}. {} concepts created, {} linked.",
        report.pages, report.items, report.concepts_created, report.concepts_linked
    );
    let usage = &report.usage;
    if !usage.models.is_empty() {
        text.push_str(&format!(
            " Tokens: {}. Cost {}.",
            tokens_by_model(usage),
            cost_text(usage.cost_usd)
        ));
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
