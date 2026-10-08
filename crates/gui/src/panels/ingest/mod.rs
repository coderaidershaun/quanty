//! The panel that adds a chapter PDF to the library: the form, then what the one ingest of the
//! app is doing.

mod books;
mod form;
mod status;

use std::path::PathBuf;

use eframe::egui;

use self::books::{BookChoice, Offer};
use crate::contract::{Catalogue, IngestRequest, Intent};
use crate::panels::PanelCx;
use crate::state::{IngestJob, Shared};
use crate::widgets;

/// The column is never wider than this, however wide the window.
const COLUMN_WIDTH: f32 = 720.0;

#[derive(Debug, Default)]
pub struct Local {
    pdf: Option<PathBuf>,
    book: BookChoice,
    author: String,
    tags: String,
    seen_picks: u64,
}

impl Local {
    fn draft(&self) -> Option<IngestRequest> {
        let pdf = self.pdf.clone()?;
        let book = match &self.book {
            BookChoice::Unchosen => return None,
            BookChoice::Existing(title) => title.as_str(),
            BookChoice::New(text) => text.trim(),
        };
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

    fn is_untouched(&self) -> bool {
        self.pdf.is_none() && self.book == BookChoice::Unchosen
    }

    fn fill_from(&mut self, request: &IngestRequest, catalogue: Option<&Catalogue>) {
        let is_in_library =
            catalogue.is_some_and(|catalogue| books::has_titled(catalogue, &request.book));
        self.pdf = Some(request.pdf.clone());
        self.book = if is_in_library {
            BookChoice::Existing(request.book.clone())
        } else {
            BookChoice::New(request.book.clone())
        };
        self.author = request.author.clone().unwrap_or_default();
        self.tags = request.tags.join(", ");
    }

    fn choose(&mut self, offer: &Offer<'_>) {
        self.book = BookChoice::Existing(offer.title.to_owned());
        self.author = offer.author.unwrap_or_default().to_owned();
        self.tags = offer.tags.join(", ");
    }

    /// Nothing is carried over from the book that was chosen before.
    fn start_new_book(&mut self) {
        self.book = BookChoice::New(String::new());
        self.author.clear();
        self.tags.clear();
    }

    /// A check that was made for another request is cleared, because the form changed after it.
    fn follow(&mut self, shared: &Shared, intents: &mut Vec<Intent>) -> Option<IngestRequest> {
        if shared.cues.pdf_picks != self.seen_picks {
            self.seen_picks = shared.cues.pdf_picks;
            self.pdf.clone_from(&shared.cues.picked_pdf);
        }
        let held = request_of(&shared.ingest);
        let catalogue = &shared.library.catalogue;
        if let Some(request) = held
            && self.is_untouched()
            && !catalogue.is_loading()
        {
            self.fill_from(request, catalogue.ready());
        }
        // A book that left the library, after a delete, stays as the title of a new book.
        if let BookChoice::Existing(title) = &self.book
            && catalogue
                .ready()
                .is_some_and(|catalogue| !books::has_titled(catalogue, title))
        {
            self.book = BookChoice::New(title.clone());
        }
        let draft = self.draft();
        let is_checked = matches!(
            shared.ingest,
            IngestJob::Checking { .. }
                | IngestJob::Checked { .. }
                | IngestJob::CheckFailed { .. }
                | IngestJob::Finished { .. }
        );
        // An untouched form may still be waiting for the library, so it is not a change.
        if is_checked && !self.is_untouched() && held != draft.as_ref() {
            intents.push(Intent::ClearIngest);
        }
        draft
    }
}

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
