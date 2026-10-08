//! The panel that adds a chapter PDF to the library, or saves a book before its first chapter:
//! the form, then what the one ingest of the app is doing.

mod books;
mod form;
mod status;

use std::path::PathBuf;

use eframe::egui;

use self::books::{BookChoice, Offer};
use crate::contract::{Catalogue, IngestRequest, Intent, NewBook, is_same_title};
use crate::panels::PanelCx;
use crate::panels::labels::{author_label, author_text, tag_labels, tags_text};
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
    seen_saves: u64,
}

impl Local {
    /// A typed title that names a book of the library is sent as the library has it, so a
    /// chapter never starts a second book that differs only in capitals.
    fn draft(&self, catalogue: Option<&Catalogue>) -> Option<IngestRequest> {
        let pdf = self.pdf.clone()?;
        let book = match &self.book {
            BookChoice::Unchosen => return None,
            BookChoice::Existing(title) => title.as_str(),
            BookChoice::New(text) => {
                let typed = text.trim();
                catalogue
                    .and_then(|catalogue| catalogue.stored_title(typed))
                    .unwrap_or(typed)
            }
        };
        if book.is_empty() {
            return None;
        }
        Some(IngestRequest {
            pdf,
            book: book.to_owned(),
            author: author_label(&self.author),
            tags: tag_labels(&self.tags),
        })
    }

    /// There is a book to save only while the form holds a typed title that is not blank.
    fn book_to_save(&self) -> Option<NewBook> {
        let BookChoice::New(text) = &self.book else {
            return None;
        };
        let title = text.trim();
        (!title.is_empty()).then(|| NewBook {
            title: title.to_owned(),
            author: author_label(&self.author),
            tags: tag_labels(&self.tags),
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
        self.author = author_text(request.author.as_deref());
        self.tags = tags_text(&request.tags);
    }

    fn choose(&mut self, offer: &Offer<'_>) {
        self.book = BookChoice::Existing(offer.title.to_owned());
        self.author = author_text(offer.author);
        self.tags = tags_text(&offer.tags);
    }

    /// Nothing is carried over from the book that was chosen before.
    fn start_new_book(&mut self) {
        self.book = BookChoice::New(String::new());
        self.author.clear();
        self.tags.clear();
    }

    /// The chosen file stays: it was not typed for the new book, and a form with no file and no
    /// book is filled again from the last check.
    fn cancel_new_book(&mut self) {
        self.book = BookChoice::Unchosen;
        self.author.clear();
        self.tags.clear();
    }

    /// Chooses the book that was just saved, once the catalogue that holds it has arrived. The
    /// cue is used up whether or not the form still shows that title, so a title typed later
    /// does not choose the book again.
    fn choose_saved_book(&mut self, shared: &Shared) {
        if shared.cues.book_saves == self.seen_saves {
            return;
        }
        let Some(saved) = shared.cues.saved_book.as_deref() else {
            return;
        };
        let Some(catalogue) = shared.library.catalogue.ready() else {
            return;
        };
        let offers = books::offers(catalogue);
        let Some(offer) = books::offer_titled(&offers, saved) else {
            return;
        };
        self.seen_saves = shared.cues.book_saves;
        if matches!(&self.book, BookChoice::New(text) if is_same_title(text, saved)) {
            self.choose(offer);
        }
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
        self.choose_saved_book(shared);
        // A book that left the library, after a delete, stays as the title of a new book.
        if let BookChoice::Existing(title) = &self.book
            && catalogue
                .ready()
                .is_some_and(|catalogue| !books::has_titled(catalogue, title))
        {
            self.book = BookChoice::New(title.clone());
        }
        let draft = self.draft(catalogue.ready());
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
