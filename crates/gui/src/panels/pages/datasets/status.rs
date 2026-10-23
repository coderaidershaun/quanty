//! What follows the form: the Start button and its cost, the running build, and the way it
//! ended, each with the buttons that go on from there.

use eframe::egui;

use super::form;
use crate::panels::PanelCx;
use crate::panels::labels::{
    START_OFF_WHILE_DELETING, TRY_AGAIN, cost_text, detail, spent_text, tokens_by_model,
};
use crate::state::{DatasetJob, Shared};
use crate::theme::{TextRole, color, space};
use crate::widgets::{self, Button, Notice};
use contract::{DatasetProgress, DatasetReport, DatasetRequest, DatasetStage, Failure, Intent};

const START: &str = "Start build";
const COST: &str =
    "Paid work: the selected model writes the rewrites and the answers; a dry run is free.";
const START_OFF_WITHOUT_NAME: &str = "Type a name for the dataset first.";
const START_OFF_WHILE_RUNNING: &str =
    "Background work is running on the server. Wait for it to end, then reload this page.";
const STARTING: &str = "Starting the build";
const KEEP_OPEN: &str = "This build continues on the server if you close the tab. If it stops, start the same build again: answers already written are not paid for twice.";
const FAILED: &str = "The build failed";
const BUILD_ANOTHER: &str = "Build another";

/// `draft` is what the form holds now.
pub(super) fn show(ui: &mut egui::Ui, draft: &DatasetRequest, cx: &mut PanelCx<'_>) {
    match &cx.shared.datasets {
        DatasetJob::Idle => {
            start_button(ui, START, draft, cx);
            ui.add_space(space::SM);
            ui.label(TextRole::Small.rich(COST));
        }
        DatasetJob::Running { progress, .. } => running(ui, progress.as_ref()),
        DatasetJob::Finished { result, .. } => finished(ui, draft, cx, result),
    }
}

/// The request a click would start, or the reason the button is off.
fn startable(draft: &DatasetRequest, shared: &Shared) -> Result<DatasetRequest, &'static str> {
    let request = form::request_of(draft).ok_or(START_OFF_WITHOUT_NAME)?;
    if shared.can_start_dataset_build() {
        Ok(request)
    } else if !shared.health.facts.running.is_empty() {
        Err(START_OFF_WHILE_RUNNING)
    } else {
        Err(START_OFF_WHILE_DELETING)
    }
}

fn start_button(ui: &mut egui::Ui, label: &str, draft: &DatasetRequest, cx: &mut PanelCx<'_>) {
    let startable = startable(draft, cx.shared);
    let button = ui.add_enabled(startable.is_ok(), Button::primary(label));
    match startable {
        Ok(request) if button.clicked() => cx.intents.push(Intent::StartDatasetBuild(request)),
        Ok(_) => {}
        Err(reason) => {
            button.on_disabled_hover_text(reason);
        }
    }
}

fn stage_words(stage: Option<DatasetStage>) -> String {
    let Some(stage) = stage else {
        return STARTING.to_owned();
    };
    let words = match stage {
        DatasetStage::Exporting { done, total } => {
            format!("Exporting the documents — {done} of {total}")
        }
        DatasetStage::Checking => "Checking the sections".to_owned(),
        DatasetStage::Planning => "Planning what to write".to_owned(),
        DatasetStage::WritingSynthetic { done, total } => {
            format!("Writing the synthetic texts — {done} of {total}")
        }
        DatasetStage::WritingSft { done, total } => {
            format!("Writing the fine-tuning examples — {done} of {total}")
        }
        DatasetStage::WritingManifest => "Writing the manifest".to_owned(),
    };
    match stage.share_done() {
        Some(share) => format!("{words}, about {}%", (share * 100.0).round()),
        None => words,
    }
}

fn running(ui: &mut egui::Ui, progress: Option<&DatasetProgress>) {
    let stage = progress.map(|progress| progress.stage);
    let words = stage_words(stage);
    ui.horizontal(|ui| {
        ui.label(TextRole::BodyStrong.rich(&words));
        if let Some(progress) = progress.filter(|progress| progress.spent.tokens() > 0) {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let spent = format!("{} so far", spent_text(&progress.spent));
                ui.label(TextRole::Small.rich(spent).color(color::TEXT_MUTED));
            });
        }
    });
    ui.add_space(space::XS);
    match stage.and_then(DatasetStage::share_done) {
        Some(share) => widgets::progress_bar(ui, &words, share),
        None => widgets::indeterminate_bar(ui, &words),
    };
    ui.add_space(space::SM);
    ui.label(TextRole::Small.rich(KEEP_OPEN));
}

fn finished(
    ui: &mut egui::Ui,
    draft: &DatasetRequest,
    cx: &mut PanelCx<'_>,
    result: &Result<DatasetReport, Failure>,
) {
    match result {
        Ok(report) => {
            let mut body = report.summary();
            if !report.usage.models.is_empty() {
                body.push_str(&format!(
                    " Tokens: {}. Cost {}.",
                    tokens_by_model(&report.usage),
                    cost_text(report.usage.cost_usd)
                ));
            }
            body.push_str(&format!(" Saved in {}.", report.folder.display()));
            Notice::success(&format!("Built {}", report.name))
                .body(&body)
                .show(ui);
        }
        Err(failure) => {
            Notice::error(FAILED).body(&failure.hint).show(ui);
            detail(ui, failure);
        }
    }
    ui.add_space(space::MD);
    ui.horizontal(|ui| {
        if result.is_err() {
            start_button(ui, TRY_AGAIN, draft, cx);
        }
        if ui.add(Button::secondary(BUILD_ANOTHER)).clicked() {
            cx.intents.push(Intent::ClearDatasetBuild);
        }
    });
}
