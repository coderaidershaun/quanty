//! The panel that adds a PDF of a media to the library, or saves a media before its first PDF:
//! the media, then the PDF, then what the one ingest of the app is doing.

mod document;
mod media;
mod status;

use std::path::PathBuf;

use eframe::egui;

use self::document::DocumentFields;
use self::media::MediaChoice;
use crate::contract::{Catalogue, Category, IngestRequest, Intent, Media, is_same_name};
use crate::panels::PanelCx;
use crate::panels::media_card::MediaFields;
use crate::state::{IngestJob, MediaEditing, Shared};
use crate::theme::{TextRole, space};
use crate::widgets;

/// The column is never wider than this, however wide the window.
const COLUMN_WIDTH: f32 = 720.0;

#[derive(Debug, Default)]
pub struct Local {
    pdf: Option<PathBuf>,
    media: MediaChoice,
    /// The PDF section as typed.
    typed: DocumentFields,
    /// The PDF section as it was when none of its boxes had the keyboard. A draft is made of this,
    /// so a check never starts on a keystroke.
    settled: DocumentFields,
    /// A pick of a PDF and a choice of a media ask for a check even when the draft is the same.
    wants_check: bool,
    seen_picks: u64,
    seen_saves: u64,
}

/// What a draft lacks before it can be checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Missing {
    Media,
    Pdf,
    Chapter,
    Title,
}

impl Missing {
    fn hint(self) -> &'static str {
        match self {
            Missing::Media => "Choose a media for this PDF, or add a new one.",
            Missing::Pdf => "Choose a PDF. It is checked as soon as it is chosen.",
            Missing::Chapter => "Type the chapter number and the chapter name of this PDF.",
            Missing::Title => "Type a title for this PDF.",
        }
    }
}

impl Local {
    fn draft(&self) -> Result<IngestRequest, Missing> {
        self.draft_of(&self.settled)
    }

    /// A PDF can be ingested only into a media of the library. Its name and its own tags come from
    /// `fields`, the settled or the typed fields of the PDF section.
    fn draft_of(&self, fields: &DocumentFields) -> Result<IngestRequest, Missing> {
        let (title, category) = self.media.chosen().ok_or(Missing::Media)?;
        let pdf = self.pdf.clone().ok_or(Missing::Pdf)?;
        let name = fields.name(category)?;
        Ok(IngestRequest {
            pdf,
            media: title.to_owned(),
            category,
            name,
            tags: fields.tags(),
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
            MediaChoice::New(MediaFields {
                category: request.category,
                title: request.media.clone(),
                ..MediaFields::default()
            })
        } else {
            MediaChoice::Existing {
                title: request.media.clone(),
                category: request.category,
            }
        };
        self.typed = DocumentFields::of(request);
        self.settled = self.typed.clone();
    }

    /// A pick is not typing, so the chapter fields that it fills in are settled at once.
    fn pick(&mut self, pdf: PathBuf) {
        self.typed.take_chapter_of(&pdf);
        self.settled.take_chapter_of(&pdf);
        self.pdf = Some(pdf);
        self.wants_check = true;
    }

    fn choose(&mut self, media: &Media) {
        if let Some(title) = &media.title {
            self.media = MediaChoice::Existing {
                title: title.clone(),
                category: media.category,
            };
            self.prefill_the_title();
            self.wants_check = true;
        }
    }

    /// A PDF of a paper or another media is named after its media until a person types a title.
    fn prefill_the_title(&mut self) {
        let Some((title, category)) = self.media.chosen() else {
            return;
        };
        if category != Category::Book {
            let title = title.to_owned();
            self.typed.title.clone_from(&title);
            self.settled.title = title;
        }
    }

    /// The media stays chosen, so the next PDF of it needs only its file and its name.
    fn add_another_pdf(&mut self) {
        self.pdf = None;
        self.typed = DocumentFields::default();
        self.settled = DocumentFields::default();
        self.prefill_the_title();
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
        if matches!(&self.media, MediaChoice::New(fields) if is_same_name(&fields.title, saved)) {
            self.choose(media);
        }
    }

    /// The form of an edit closes once its save went through, and stays open with what was typed
    /// when the save was refused. It reads the state and not an event, so an answer that came
    /// while another tab was open is found when the page is drawn again.
    fn follow_the_edit(&mut self, media_edit: &MediaEditing) {
        let MediaChoice::Editing {
            title,
            fields,
            is_sent,
            ..
        } = &mut self.media
        else {
            return;
        };
        if !*is_sent || media_edit.is_saving() {
            return;
        }
        let is_refused =
            matches!(media_edit, MediaEditing::Failed { edit, .. } if edit.title == *title);
        if is_refused {
            *is_sent = false;
            return;
        }
        let saved = MediaChoice::Existing {
            title: std::mem::take(title),
            category: fields.category,
        };
        self.media = saved;
    }

    /// A chosen media takes its category from the library, where an edit may have changed it. A
    /// media that left the library, after a delete, stays as the title of a new media.
    fn follow_the_chosen_media(&mut self, catalogue: &Catalogue) {
        let (MediaChoice::Existing { title, category }
        | MediaChoice::Editing {
            title, category, ..
        }) = &mut self.media
        else {
            return;
        };
        if let Some(stored) = media::titled(catalogue, title) {
            *category = stored.category;
            return;
        }
        let fields = MediaFields {
            category: *category,
            title: std::mem::take(title),
            ..MediaFields::default()
        };
        self.media = MediaChoice::New(fields);
    }

    fn follow(&mut self, shared: &Shared) {
        if shared.cues.pdf_picks != self.seen_picks {
            self.seen_picks = shared.cues.pdf_picks;
            if let Some(pdf) = &shared.cues.picked_pdf {
                self.pick(pdf.clone());
            }
        }
        let catalogue = &shared.library.catalogue;
        if let Some(request) = request_of(&shared.ingest)
            && self.is_untouched()
            && !catalogue.is_loading()
        {
            self.fill_from(request, catalogue.ready());
        }
        self.choose_saved_media(shared);
        self.follow_the_edit(&shared.library.media_edit);
        if let Some(catalogue) = catalogue.ready() {
            self.follow_the_chosen_media(catalogue);
        }
    }

    /// The state takes the check at the end of this frame, so the next frame holds the draft and
    /// asks for nothing. An untouched form may still be waiting for the library, so the request
    /// it holds is not cleared.
    fn ask_for_a_check(
        &mut self,
        draft: &Result<IngestRequest, Missing>,
        job: &IngestJob,
        intents: &mut Vec<Intent>,
    ) {
        let wants_check = std::mem::take(&mut self.wants_check);
        if job.is_running() {
            return;
        }
        let held = request_of(job);
        match draft {
            Ok(draft) if wants_check || held != Some(draft) => {
                intents.push(Intent::CheckIngest(draft.clone()));
            }
            Err(_) if held.is_some() && !self.is_untouched() => intents.push(Intent::ClearIngest),
            Ok(_) | Err(_) => {}
        }
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
    local.follow(cx.shared);
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
                    ui.label(TextRole::Heading.rich("Add media"));
                    ui.add_space(space::MD);
                    ui.add_enabled_ui(!cx.shared.ingest.is_running(), |ui| {
                        media::show(ui, local, cx);
                        ui.add_space(space::LG);
                        document::show(ui, local, cx.intents);
                    });
                    ui.add_space(space::LG);
                    let draft = local.draft();
                    local.ask_for_a_check(&draft, &cx.shared.ingest, cx.intents);
                    status::show(ui, local, cx, &draft);
                });
        });
    });
}
