//! The Library page: the stored media with their documents, and what a person can do with each
//! media and each document.

mod document;
mod media;
mod media_form;
mod tags_form;

use eframe::egui;

use crate::contract::{Catalogue, Intent, Loadable};
use crate::panels::PanelCx;
use crate::state::{Library, Shared};
use crate::theme::{Icon, TextRole, space};
use crate::widgets::{self, Placeholder};

/// The column is never wider than this, however wide the window.
const COLUMN_WIDTH: f32 = 960.0;

const TITLE: &str = "Library";
const LOADING: &str = "Reading the library";
const FAILED: &str = "The library did not load";
const TRY_AGAIN: &str = "Try again";
const EMPTY: &str = "Your library is empty";
const EMPTY_HINT: &str = "Add media on the Ingest tab.";
const INGEST_RUNNING: &str = "An ingest is running. Media and tags can be edited when it is done.";
const SAVE: &str = "Save";
const CANCEL: &str = "Cancel";

/// One form is open at a time, so opening a second one drops what was typed in the first.
#[derive(Debug, Default)]
pub struct Local {
    form: Option<Form>,
}

#[derive(Debug)]
enum Form {
    Tags(tags_form::Draft),
    Media(media_form::Draft),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveStep {
    Typing,
    Sent,
    Refused,
}

/// A form or its refusal near the bottom of the list would open out of sight, so it is scrolled
/// into view: once, so that the person can still scroll away, and only as far as needed, so that
/// a form already on screen does not move. Call it last, because the form ends where the drawing
/// ends.
fn reveal(ui: &egui::Ui, form_top: f32, should_reveal: &mut bool) {
    if !std::mem::take(should_reveal) {
        return;
    }
    let drawn = ui.min_rect();
    let form = egui::Rect::from_x_y_ranges(drawn.x_range(), form_top..=drawn.bottom());
    ui.scroll_to_rect(form, None);
}

/// A save on its way must not lose its form, and a draft must not be made from labels that a
/// catalogue on its way is about to replace.
fn can_edit(shared: &Shared) -> bool {
    can_send_media_edit(shared)
        && shared.library.busy.is_empty()
        && shared.library.pending.is_none()
}

/// The app ignores an edit of a media while an ingest runs or a media is being saved or edited. A
/// form that sent such an edit would close as if it was saved, so its Save is off then.
// SMELL: the state ignores the edit by these same rules, written again there. A change to one must
// be made in both, or a save is dropped with no word to the person.
fn can_send_media_edit(shared: &Shared) -> bool {
    !shared.ingest.is_running()
        && !shared.library.media_save.is_saving()
        && !shared.library.media_edit.is_saving()
}

/// Each form reads the state and not an event, so an answer that came while another tab was open
/// is found when the page is drawn again.
fn follow(form: &mut Option<Form>, library: &Library) {
    let stays_open = match form {
        None => return,
        Some(Form::Tags(draft)) => tags_form::follow(draft, library),
        Some(Form::Media(draft)) => media_form::follow(draft, library),
    };
    if !stays_open {
        *form = None;
    }
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    follow(&mut local.form, &cx.shared.library);
    let page = ui.max_rect();
    let width = page.width().min(COLUMN_WIDTH);
    let column = egui::Rect::from_min_size(
        egui::pos2(page.center().x - width / 2.0, page.top()),
        egui::vec2(width, page.height()),
    );
    let frame = widgets::panel_frame();
    let margin = frame.total_margin().sum();
    let builder = egui::UiBuilder::new()
        .id_salt("library_column")
        .max_rect(column)
        .layout(egui::Layout::top_down(egui::Align::Min));
    let shared = cx.shared;
    let listed = shared
        .library
        .catalogue
        .ready()
        .filter(|catalogue| !catalogue.media.is_empty());
    ui.scope_builder(builder, |ui| {
        frame.show(ui, |ui| {
            ui.set_width(column.width() - margin.x);
            if listed.is_none() {
                // What is said in place of a list puts itself in the middle of the room it is
                // given, so the frame takes the whole column first. With a list, the frame ends
                // where the list ends.
                ui.set_min_height(ui.available_height());
            }
            ui.label(TextRole::Heading.rich(TITLE));
            if shared.ingest.is_running() {
                ui.label(TextRole::Small.rich(INGEST_RUNNING));
            }
            ui.add_space(space::MD);
            match listed {
                Some(catalogue) => media_list(ui, catalogue, local, cx),
                None => nothing_listed(ui, &shared.library.catalogue, cx.intents),
            }
        });
    });
}

fn nothing_listed(ui: &mut egui::Ui, catalogue: &Loadable<Catalogue>, intents: &mut Vec<Intent>) {
    match catalogue {
        Loadable::Idle | Loadable::Loading => {
            Placeholder::loading(LOADING).show(ui);
        }
        Loadable::Failed(failure) => {
            let retry = Placeholder::error(FAILED)
                .hint(&failure.hint)
                .action(TRY_AGAIN)
                .show(ui);
            if retry.action_clicked {
                intents.push(Intent::RefreshCatalogue);
            }
        }
        Loadable::Ready(_) => {
            Placeholder::empty(Icon::LIBRARY, EMPTY)
                .hint(EMPTY_HINT)
                .show(ui);
        }
    }
}

fn media_list(ui: &mut egui::Ui, catalogue: &Catalogue, local: &mut Local, cx: &mut PanelCx<'_>) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .max_height(ui.available_height())
        .show(ui, |ui| {
            for media in &catalogue.media {
                media::show(ui, media, local, cx);
                ui.add_space(space::LG);
            }
        });
}
