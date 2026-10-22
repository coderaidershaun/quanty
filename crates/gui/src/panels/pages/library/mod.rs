//! The Library page: the shelf of media on the left, the page of the open media or document on the
//! right, what is said when there is nothing to list, and the sheet that asks before a delete.

mod concepts;
mod contents;
mod delete;
mod document;
mod insights;
mod media;
mod media_form;
mod overview;
mod shelf;
mod tags_form;

use eframe::egui;

use crate::panels::PanelCx;
use crate::panels::labels::TRY_AGAIN;
use crate::panels::seam::{column, framed};
use crate::state::{Library, Open, Shared};
use crate::theme::{Icon, TextRole, space};
use crate::widgets::Placeholder;
use contract::{Catalogue, Document, Intent, Loadable, Media};

/// The column of what is said in place of a list is never wider than this, however wide the
/// window.
const COLUMN_WIDTH: f32 = 960.0;
const SHELF_WIDTH: f32 = 300.0;
/// The page of the open item is never wider than this, and the desk shows to its right.
const PAGE_WIDTH: f32 = 960.0;

const TITLE: &str = "Library";
const LOADING: &str = "Reading the library";
const FAILED: &str = "The library did not load";
const EMPTY: &str = "Your library is empty";
const EMPTY_HINT: &str = "Add media on the Ingest tab.";
const NOTHING_OPEN: &str = "Nothing is open";
const NOTHING_OPEN_HINT: &str = "Choose a media or a chapter on the shelf.";
const INGEST_RUNNING: &str = "An ingest is running. Media and tags can be edited, and documents and media deleted, when it is done.";
const SAVE: &str = "Save";
const CANCEL: &str = "Cancel";

/// One form is open at a time, so opening a second one drops what was typed in the first. A delete
/// closes the open form when it is confirmed, not when its sheet is cancelled: in an open form, a
/// delete that failed would show as a refused save or not at all.
#[derive(Debug, Default)]
pub struct Local {
    find: String,
    rows: shelf::Rows,
    form: Option<Form>,
    question: Option<delete::Question>,
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

/// A save on its way must not lose its form, and a draft must not be made from labels that a
/// catalogue on its way is about to replace. A delete on its way is a change too, and no other
/// change or delete starts beside it. A build only reads the library, so it does not hold these
/// forms back.
fn can_edit(shared: &Shared) -> bool {
    shared.can_edit_media() && shared.library.busy.is_empty() && shared.library.pending.is_none()
}

/// A delete also waits for the build that reads the documents it would remove.
fn can_delete(shared: &Shared) -> bool {
    shared.can_delete() && shared.library.pending.is_none()
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
    delete::follow(&mut local.question, cx.shared);
    let shared = cx.shared;
    let listed = shared
        .library
        .catalogue
        .ready()
        .filter(|catalogue| !catalogue.media.is_empty());
    match listed {
        Some(catalogue) => panes(ui, catalogue, local, cx),
        None => column(ui, "library_column", COLUMN_WIDTH, |ui, _| {
            // What is said in place of a list puts itself in the middle of the room it is given,
            // so the frame takes the whole column first.
            ui.set_min_height(ui.available_height());
            ui.label(TextRole::Title.rich(TITLE));
            if shared.ingest.is_running() {
                ui.label(TextRole::Small.rich(INGEST_RUNNING));
            }
            ui.add_space(space::MD);
            nothing_listed(ui, &shared.library.catalogue, cx.intents);
        }),
    }
    if delete::sheet(ui.ctx(), &mut local.question, cx.intents) {
        local.form = None;
    }
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

/// Two sheets that fill the room: the shelf, and beside it the page of what is open.
fn panes(ui: &mut egui::Ui, catalogue: &Catalogue, local: &mut Local, cx: &mut PanelCx<'_>) {
    let room = ui.max_rect();
    let shelf_width = SHELF_WIDTH.min(room.width());
    let shelf = egui::Rect::from_min_size(room.min, egui::vec2(shelf_width, room.height()));
    let page_left = shelf.right() + space::MD;
    let page_width = (room.right() - page_left).clamp(0.0, PAGE_WIDTH);
    let page = egui::Rect::from_min_size(
        egui::pos2(page_left, room.top()),
        egui::vec2(page_width, room.height()),
    );
    framed(ui, "library_shelf", shelf, |ui, height| {
        ui.set_min_height(height);
        shelf::show(ui, local, catalogue, cx);
    });
    framed(ui, "library_page", page, |ui, height| {
        ui.set_min_height(height);
        open_page(ui, catalogue, local, cx);
    });
}

fn open_page(ui: &mut egui::Ui, catalogue: &Catalogue, local: &mut Local, cx: &mut PanelCx<'_>) {
    if cx.shared.ingest.is_running() {
        ui.label(TextRole::Small.rich(INGEST_RUNNING));
        ui.add_space(space::SM);
    }
    match opened(catalogue, cx.shared.shelf.open.as_ref()) {
        Some(Opened::Media(media)) => media::show(ui, media, local, cx),
        Some(Opened::Document(media, document)) => document::show(ui, media, document, local, cx),
        None => {
            Placeholder::empty(Icon::LIBRARY, NOTHING_OPEN)
                .hint(NOTHING_OPEN_HINT)
                .show(ui);
        }
    }
}

/// What the page shows. A document comes with its media, for the line that names it.
enum Opened<'a> {
    Media(&'a Media),
    Document(&'a Media, &'a Document),
}

/// `None` when nothing is open, or the open item is not in the catalogue.
fn opened<'a>(catalogue: &'a Catalogue, open: Option<&Open>) -> Option<Opened<'a>> {
    match open? {
        Open::Media { title } => catalogue
            .media
            .iter()
            .find(|media| media.title == *title)
            .map(Opened::Media),
        Open::Document(doc) => {
            let media = catalogue.media_of(*doc)?;
            let document = media
                .documents
                .iter()
                .find(|document| document.id == *doc)?;
            Some(Opened::Document(media, document))
        }
    }
}
