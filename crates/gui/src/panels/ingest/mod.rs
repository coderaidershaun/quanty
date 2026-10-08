//! The panel that adds a PDF of a media to the library, or saves a media before its first PDF:
//! the form, then what the one ingest of the app is doing.

mod form;
mod media;
mod status;

use std::path::{Path, PathBuf};

use eframe::egui;

use self::media::{MediaChoice, NewMediaForm};
use crate::contract::{
    Catalogue, Category, DocumentName, IngestRequest, Intent, Media, is_same_title,
};
use crate::panels::PanelCx;
use crate::panels::labels::{list_of, text_of};
use crate::state::{IngestJob, Shared};
use crate::widgets;

/// The column is never wider than this, however wide the window.
const COLUMN_WIDTH: f32 = 720.0;

#[derive(Debug, Default)]
pub struct Local {
    pdf: Option<PathBuf>,
    media: MediaChoice,
    /// The tags of this PDF only, as typed.
    own_tags: String,
    seen_picks: u64,
    seen_saves: u64,
}

fn file_name_of(pdf: &Path) -> String {
    pdf.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl Local {
    /// A PDF can be ingested only into a media of the library. A book chapter is named by its
    /// file name, and there is no request while the file name is not one; a paper or another
    /// media names its document after the media.
    fn draft(&self) -> Option<IngestRequest> {
        let pdf = self.pdf.clone()?;
        let MediaChoice::Existing { title, category } = &self.media else {
            return None;
        };
        let name = match category {
            Category::Book => DocumentName::from_chapter_file_name(&file_name_of(&pdf))?,
            Category::Paper | Category::Other => DocumentName::Title(title.clone()),
        };
        Some(IngestRequest {
            pdf,
            media: title.clone(),
            category: *category,
            name,
            tags: list_of(&self.own_tags),
        })
    }

    fn is_untouched(&self) -> bool {
        self.pdf.is_none() && self.media == MediaChoice::Unchosen
    }

    /// A media that a ready library does not hold is offered as a new media of that title.
    fn fill_from(&mut self, request: &IngestRequest, catalogue: Option<&Catalogue>) {
        let is_gone =
            catalogue.is_some_and(|catalogue| media::titled(catalogue, &request.media).is_none());
        self.pdf = Some(request.pdf.clone());
        self.media = if is_gone {
            MediaChoice::New(NewMediaForm {
                category: request.category,
                title: request.media.clone(),
                ..NewMediaForm::default()
            })
        } else {
            MediaChoice::Existing {
                title: request.media.clone(),
                category: request.category,
            }
        };
        self.own_tags = text_of(&request.tags);
    }

    fn choose(&mut self, media: &Media) {
        if let Some(title) = &media.title {
            self.media = MediaChoice::Existing {
                title: title.clone(),
                category: media.category,
            };
        }
    }

    fn start_new_media(&mut self) {
        self.media = MediaChoice::New(NewMediaForm::default());
    }

    /// The chosen file and the tags for it stay: they were not typed for the new media.
    fn cancel_new_media(&mut self) {
        self.media = MediaChoice::Unchosen;
    }

    /// Chooses the media that was just saved, once the catalogue that holds it has arrived. The
    /// cue is used up whether or not the form still shows that title, so a title typed later
    /// does not choose the media again.
    fn choose_saved_media(&mut self, shared: &Shared) {
        if shared.cues.media_saves == self.seen_saves {
            return;
        }
        let Some(saved) = shared.cues.saved_media.as_deref() else {
            return;
        };
        let Some(catalogue) = shared.library.catalogue.ready() else {
            return;
        };
        let Some(media) = catalogue.media_titled(saved) else {
            return;
        };
        self.seen_saves = shared.cues.media_saves;
        if matches!(&self.media, MediaChoice::New(form) if is_same_title(&form.title, saved)) {
            self.choose(media);
        }
    }

    /// A chosen media takes its category from the library, where an edit may have changed it. A
    /// media that left the library, after a delete, stays as the title of a new media.
    fn follow_the_chosen_media(&mut self, catalogue: &Catalogue) {
        let MediaChoice::Existing { title, category } = &mut self.media else {
            return;
        };
        match media::titled(catalogue, title) {
            Some(stored) => *category = stored.category,
            None => {
                let form = NewMediaForm {
                    category: *category,
                    title: std::mem::take(title),
                    ..NewMediaForm::default()
                };
                self.media = MediaChoice::New(form);
            }
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
        self.choose_saved_media(shared);
        if let Some(catalogue) = catalogue.ready() {
            self.follow_the_chosen_media(catalogue);
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
