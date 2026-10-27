//! What follows the PDF: what the draft still lacks, the check, the running ingest and the way it
//! ended, each with the buttons that go on from there.

use eframe::egui;

use super::{Local, Missing};
use crate::panels::PanelCx;
use crate::panels::labels::{
    COPY_COMMAND, START_OFF_WHILE_DELETING, TRY_AGAIN, cost_text, detail, tokens_by_model,
};
use crate::panels::pages::progress::stage_row;
use crate::state::IngestJob;
use crate::theme::{TextRole, space};
use crate::widgets::{self, Button, Notice};
use contract::{
    ChapterInsights, ChapterState, DocId, Failure, IngestOutcome, IngestProgress, IngestReport,
    IngestRequest, IngestStage, InsightsOutcome, Intent, Preflight, Usage,
};

const CHECKING: &str = "Checking the PDF";
const CONNECTING: &str = "Connecting to the stores";
const COST: &str = "Checking is free. Starting is paid work: the selected agent and Jev convert each page, Gemini embeds the items, and the selected agent writes artifact descriptions, the insights, takeaways and cheat sheet and reads the concepts.";
const KEEP_OPEN: &str = "This ingest continues on the server if you close the tab. If it stops, start the same PDF again: pages that are converted are not paid for twice.";

/// `draft` is what the form holds now, made of its settled fields.
pub(super) fn show(
    ui: &mut egui::Ui,
    local: &mut Local,
    cx: &mut PanelCx<'_>,
    draft: &Result<IngestRequest, Missing>,
) {
    for job in &cx.shared.health.facts.running {
        let message = match job.kind {
            contract::JobKind::Ingest => format!(
                "An ingest is running on the server: {}. Wait for it to end.",
                job.name
            ),
            contract::JobKind::Dataset | contract::JobKind::Insights => format!(
                "Background work is running on the server: {}. Wait for it to end.",
                job.name
            ),
        };
        ui.label(TextRole::Small.rich(ui, message));
        ui.label(TextRole::Small.rich(
            ui,
            "Reload this page after it ends to enable another start.",
        ));
    }
    match &cx.shared.ingest {
        // A draft that is complete is checked in this frame, so it needs no words.
        IngestJob::Idle => {
            if let Err(missing) = draft {
                ui.label(TextRole::Small.rich(ui, missing.hint()));
            }
        }
        IngestJob::Checking { .. } => {
            widgets::indeterminate_bar(ui, CHECKING);
            ui.label(TextRole::Small.rich(ui, CHECKING));
        }
        IngestJob::Checked { request, preflight } if preflight.blockers.is_empty() => {
            checked_line(ui, preflight);
            ui.add_space(space::MD);
            // A box may still have the keyboard, so Start reads the boxes as typed. A typed change
            // that makes another request is not checked yet, and a start would run the old one.
            let can_start = local.draft_of(&local.typed).as_ref() == Ok(request);
            // A delete removes what a start would write, so the two never run together.
            let is_deleting = cx.shared.library.is_deleting();
            let start = ui.add_enabled(
                can_start && !is_deleting && cx.shared.health.facts.running.is_empty(),
                Button::primary("Start ingest"),
            );
            let start = if is_deleting {
                start.on_disabled_hover_text(START_OFF_WHILE_DELETING)
            } else {
                start
            };
            if start.clicked() {
                cx.intents.push(Intent::StartIngest);
            }
            ui.add_space(space::SM);
            ui.label(TextRole::Small.rich(ui, COST));
        }
        IngestJob::Checked { preflight, .. } => {
            ui.label(TextRole::BodyStrong.rich(ui, preflight.name.label()));
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
        IngestJob::Running { progress, .. } => {
            running(ui, progress.as_ref());
        }
        IngestJob::Finished {
            request,
            result,
            spent,
        } => {
            finished(ui, local, cx.intents, request, result);
            if result.is_err() {
                ui.label(TextRole::Small.rich(ui, spent_sentence(spent)));
            }
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
            "All are converted already: embedding, missing artifact descriptions and insights, and concept extraction can incur costs."
                .to_owned(),
        ),
        Some(ChapterState::Ingested { items }) => Some(format!(
            "Already ingested with {items} items: a start converts nothing and pays for nothing."
        )),
        Some(ChapterState::IngestedWithoutMetadata { items, missing_artifacts, missing_insights }) => {
            let missing = match (missing_artifacts, missing_insights) {
                (true, true) => "artifact descriptions and insights",
                (true, false) => "artifact descriptions",
                (false, true) => "insights",
                (false, false) => return,
            };
            Some(format!("Already ingested with {items} items. A start writes the missing {missing}. Nothing is converted or embedded."))
        },
        Some(ChapterState::IngestedWithoutInsights { items }) => Some(format!(
            "Already ingested with {items} items, but its insights are not saved yet: a start pays only for the insights, takeaways and cheat sheet."
        )),
    };
    if let Some(words) = words {
        line.push_str(". ");
        line.push_str(&words);
    }
    ui.label(TextRole::Body.rich(ui, line));
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
        IngestStage::WritingArtifacts { done, items } => {
            format!("Writing artifact descriptions — {done} of {items}")
        }
        IngestStage::WritingInsights => {
            "Writing the insights, takeaways and cheat sheet".to_owned()
        }
        IngestStage::ReadingConcepts { done, items } => {
            format!("Reading the concepts — {done} of {items}")
        }
        IngestStage::LinkingConcepts { done, items } => {
            format!("Linking the concepts — {done} of {items}")
        }
    };
    match stage.share_done() {
        Some(share) => format!("{words}, about {}%", (share * 100.0).round()),
        None => words,
    }
}

fn running(ui: &mut egui::Ui, progress: Option<&IngestProgress>) {
    let stage = progress.map(|progress| progress.stage);
    let words = stage_words(stage);
    stage_row(ui, &words, progress.map(|progress| &progress.spent));
    ui.add_space(space::XS);
    match stage.and_then(IngestStage::share_done) {
        Some(share) => widgets::progress_bar(ui, &words, share),
        None => widgets::indeterminate_bar(ui, &words),
    };
    ui.add_space(space::SM);
    ui.label(TextRole::Small.rich(ui, KEEP_OPEN));
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
            let body = with_insights(summary(report), report.insights.as_ref());
            Notice::success(&format!("Ingested {}", report.title))
                .body(&body)
                .show(ui);
            not_written(ui, intents, report.doc, report.insights.as_ref());
        }
        Ok(IngestOutcome::AlreadyIngested {
            doc,
            items,
            insights,
            usage,
            artifacts,
        }) => {
            let stored = format!(
                "{items} items are stored with {artifacts} artifact descriptions. Nothing was converted or embedded."
            );
            let body = with_insights(stored, insights.as_ref()) + &spent_sentence(usage);
            Notice::info("Already ingested").body(&body).show(ui);
            not_written(ui, intents, *doc, insights.as_ref());
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
            ui.label(TextRole::Small.rich(
                ui,
                "Pages that are converted are kept and are not paid for twice.",
            ));
        }
    }
    ui.add_space(space::MD);
    ui.horizontal_wrapped(|ui| {
        if result.is_err() && ui.add(Button::primary(TRY_AGAIN)).clicked() {
            intents.push(Intent::CheckIngest(request.clone()));
        }
        if ui.add(Button::secondary("Add another PDF")).clicked() {
            intents.push(Intent::ClearIngest);
            local.add_another_pdf();
        }
    });
}

/// What became of the insights, said in one sentence, when it is not a failure: that has a notice
/// of its own.
fn insights_sentence(outcome: &InsightsOutcome) -> Option<&'static str> {
    match outcome {
        InsightsOutcome::Written => Some(
            "The insights, takeaways and cheat sheet are written: the Library shows them under Insights.",
        ),
        InsightsOutcome::AlreadySaved => {
            Some("The insights were saved already, so nothing was asked for them.")
        }
        InsightsOutcome::NoneForBookPart => {
            Some("A part of a book that is not a chapter gets no insights.")
        }
        InsightsOutcome::NotWritten { .. } => None,
    }
}

fn with_insights(mut text: String, insights: Option<&InsightsOutcome>) -> String {
    if let Some(sentence) = insights.and_then(insights_sentence) {
        text.push(' ');
        text.push_str(sentence);
    }
    text
}

/// Under the notice of an end that stored the items: why the insights were not written, and the
/// command that writes them.
fn not_written(
    ui: &mut egui::Ui,
    intents: &mut Vec<Intent>,
    doc: DocId,
    insights: Option<&InsightsOutcome>,
) {
    let Some(InsightsOutcome::NotWritten { reason }) = insights else {
        return;
    };
    let command = ChapterInsights::command_for(doc);
    let why = reason.trim_end().trim_end_matches('.');
    let body = format!(
        "Why: {why}. The items are stored, so run {command} to write them; it asks the model of the Ingest choice once, which is paid."
    );
    ui.add_space(space::SM);
    let notice = Notice::warning("The insights were not written")
        .body(&body)
        .action(COPY_COMMAND)
        .show(ui);
    if notice.action_clicked {
        intents.push(Intent::CopyText(command));
    }
}

/// " Tokens: …. Cost …." for a run that used a model, and nothing for one that used none.
fn spent_sentence(usage: &Usage) -> String {
    if usage.models.is_empty() {
        return String::new();
    }
    format!(
        " Tokens: {}. Cost {}.",
        tokens_by_model(usage),
        cost_text(usage.cost_usd)
    )
}

fn summary(report: &IngestReport) -> String {
    let mut text = format!(
        "{} pages, {}. {} concepts created, {} linked. {} artifact descriptions complete.",
        report.pages,
        report.items,
        report.concepts_created,
        report.concepts_linked,
        report.artifacts
    );
    text.push_str(&spent_sentence(&report.usage));
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
