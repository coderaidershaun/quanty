//! The panel that adds a chapter PDF to the library: the form, then what the one ingest of the
//! app is doing.

mod form;
mod status;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use eframe::egui;

use crate::contract::{IngestRequest, Intent};
use crate::panels::PanelCx;
use crate::state::{IngestJob, Shared};
use crate::widgets;

/// The column is never wider than this, however wide the window.
const COLUMN_WIDTH: f32 = 720.0;

/// What the panel keeps between frames: the form as the person filled it.
#[derive(Debug, Default)]
pub struct Local {
    pdf: Option<PathBuf>,
    book: String,
    author: String,
    tags: String,
    /// The file picks already copied from the shared state.
    seen_picks: u64,
}

impl Local {
    /// The request the form holds, or `None` until a file is chosen and the book has a title.
    fn draft(&self) -> Option<IngestRequest> {
        let pdf = self.pdf.clone()?;
        let book = self.book.trim();
        if book.is_empty() {
            return None;
        }
        let author = self.author.trim();
        Some(IngestRequest {
            pdf,
            book: book.to_owned(),
            author: (!author.is_empty()).then(|| author.to_owned()),
            tags: self
                .tags
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(str::to_owned)
                .collect(),
        })
    }

    fn fill_from(&mut self, request: &IngestRequest) {
        self.pdf = Some(request.pdf.clone());
        self.book.clone_from(&request.book);
        self.author = request.author.clone().unwrap_or_default();
        self.tags = request.tags.join(", ");
    }

    /// Brings the form up to date with the app, and returns the request it now holds. A check
    /// that was made for another request is cleared, because the form changed after it.
    fn follow(&mut self, shared: &Shared, intents: &mut Vec<Intent>) -> Option<IngestRequest> {
        if shared.cues.pdf_picks != self.seen_picks {
            self.seen_picks = shared.cues.pdf_picks;
            self.pdf.clone_from(&shared.cues.picked_pdf);
        }
        let held = request_of(&shared.ingest);
        if let Some(request) = held
            && self.pdf.is_none()
            && self.book.is_empty()
        {
            self.fill_from(request);
        }
        let draft = self.draft();
        let is_checked = matches!(
            shared.ingest,
            IngestJob::Checking { .. }
                | IngestJob::Checked { .. }
                | IngestJob::CheckFailed { .. }
                | IngestJob::Finished { .. }
        );
        if is_checked && held != draft.as_ref() {
            intents.push(Intent::ClearIngest);
        }
        draft
    }
}

/// The request the ingest of the app was asked for, in every state but `Idle`.
fn request_of(job: &IngestJob) -> Option<&IngestRequest> {
    match job {
        IngestJob::Idle => None,
        IngestJob::Checking { request, .. }
        | IngestJob::Checked { request, .. }
        | IngestJob::CheckFailed { request, .. }
        | IngestJob::Running { request, .. }
        | IngestJob::Stopping { request, .. }
        | IngestJob::Finished { request, .. } => Some(request),
    }
}

/// Draws the page into `ui`, which is the whole area under the top bar. It opens no dialog and
/// does no work: it pushes the intents that do.
pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let draft = local.follow(cx.shared, cx.intents);
    let page = ui.max_rect();
    let column = egui::Rect::from_min_size(
        egui::pos2(
            page.center().x - page.width().min(COLUMN_WIDTH) / 2.0,
            page.top(),
        ),
        egui::vec2(page.width().min(COLUMN_WIDTH), page.height()),
    );
    let frame = widgets::panel_frame();
    let margin = frame.total_margin().sum();
    let builder = egui::UiBuilder::new()
        .id_salt("ingest_column")
        .max_rect(column)
        .layout(egui::Layout::top_down(egui::Align::Min));
    ui.scope_builder(builder, |ui| {
        frame.show(ui, |ui| {
            ui.set_width(column.width() - margin.x);
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .max_height(column.height() - margin.y)
                .show(ui, |ui| {
                    form::show(ui, local, cx);
                    status::show(ui, local, cx, draft.as_ref());
                });
        });
    });
}
