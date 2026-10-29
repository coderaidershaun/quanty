//! The Library page: the shelf of media on the left, the page of the open media or document on the
//! right with the reader of a document beside it, what is said when there is nothing to list, and
//! the sheet that asks before a delete. In a room too narrow for both, the shelf and the page take
//! turns at filling it.

mod concepts;
mod contents;
mod delete;
mod document;
mod insights;
mod media;
mod media_form;
mod overview;
mod reader;
mod shelf;
mod tags_form;

use eframe::egui;

use crate::panels::labels::{NO_MEDIA, TRY_AGAIN};
use crate::panels::seam::{column, framed};
use crate::panels::{PanelCx, PanelWidth, source};
use crate::state::{Library, Open, Shared};
use crate::theme::{Icon, TextRole, space};
use crate::widgets::{ControlSize, ListRow, Placeholder};
use contract::{Catalogue, Document, Intent, Loadable, Media};

/// The column of what is said in place of a list is never wider than this, however wide the
/// window.
const COLUMN_WIDTH: f32 = 960.0;
const SHELF_WIDTH: f32 = 300.0;
/// The width of the shelf beside the page in a room too narrow for the full shelf.
const NARROW_SHELF_WIDTH: f32 = 240.0;
/// From this width of the room on, the shelf is drawn at its full width.
const FULL_SHELF_FROM: f32 = 960.0;
/// Below this width of the room, the shelf and the page take turns at filling it.
const BESIDE_FROM: f32 = 720.0;
/// From this width of the room on, the reader stands beside the page. In a narrower room it takes
/// the place of the page.
const READER_BESIDE_FROM: f32 = 1200.0;
/// A page whose content is narrower than this wraps its rows of buttons and makes the buttons tall
/// enough for a finger.
const NARROW_BELOW: f32 = 480.0;
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
    /// The title of the media whose own page fills a room too narrow for the shelf beside it. It
    /// is dropped as soon as that media is not the open item.
    media_page: Option<String>,
    /// The Source panel as the reader shows it, with its own zoom and tab.
    reader: source::Local,
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

/// The page of a media fills the room only while that media is open.
fn follow_media_page(media_page: &mut Option<String>, open: Option<&Open>) {
    if let Some(title) = media_page
        && !matches!(open, Some(Open::Media { title: Some(open_title) }) if open_title == title)
    {
        *media_page = None;
    }
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    follow(&mut local.form, &cx.shared.library);
    follow_media_page(&mut local.media_page, cx.shared.shelf.open.as_ref());
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
            ui.label(TextRole::Title.rich(ui, TITLE));
            if shared.ingest.is_running() {
                ui.label(TextRole::Small.rich(ui, INGEST_RUNNING));
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
                .action_size(width(ui).control_size())
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

/// How the shelf is drawn, by the room it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShelfFit {
    /// Beside the page, at its full width and with its rows at the usual height.
    Full,
    /// Beside the page, narrower, with rows tall enough for a finger.
    Narrow,
    /// Alone in the room, with rows tall enough for a finger and a row that opens the page of the
    /// open media.
    Alone,
}

impl ShelfFit {
    fn row_size(self) -> ControlSize {
        match self {
            ShelfFit::Full => ControlSize::Small,
            ShelfFit::Narrow | ShelfFit::Alone => ControlSize::Large,
        }
    }

    /// The find box is tall where the rows are, so that its Clear button can be pressed by a
    /// finger.
    fn find_size(self) -> ControlSize {
        match self {
            ShelfFit::Full => ControlSize::Medium,
            ShelfFit::Narrow | ShelfFit::Alone => ControlSize::Large,
        }
    }
}

/// Where a page is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// Beside the shelf.
    Beside,
    /// Alone in the room, with a row at its top that goes back.
    Alone,
}

/// Two sheets that fill the room: the shelf, and beside it the page of what is open. A reader that
/// is open stands beside the page, or takes its place in a room that is too narrow for both. A room
/// too narrow for the shelf and the page is filled by one of them.
fn panes(ui: &mut egui::Ui, catalogue: &Catalogue, local: &mut Local, cx: &mut PanelCx<'_>) {
    let room = ui.max_rect();
    let open = opened(catalogue, cx.shared.shelf.open.as_ref());
    let is_reading = cx.shared.shelf.is_reading && matches!(open, Some(Opened::Document(..)));
    if room.width() < BESIDE_FROM {
        match &open {
            Some(Opened::Document(_, document)) if is_reading => {
                reader::show_alone(ui, room, &document.name().label(), local, cx);
            }
            _ => one_at_a_time(ui, catalogue, open, local, cx),
        }
        return;
    }
    let (fit, shelf_width) = if room.width() >= FULL_SHELF_FROM {
        (ShelfFit::Full, SHELF_WIDTH)
    } else {
        (ShelfFit::Narrow, NARROW_SHELF_WIDTH)
    };
    let shelf = egui::Rect::from_min_size(room.min, egui::vec2(shelf_width, room.height()));
    let page_left = shelf.right() + space::MD;
    let rest = (room.right() - page_left).max(0.0);
    let is_reader_beside_page = is_reading && room.width() >= READER_BESIDE_FROM;
    let page_width = if is_reader_beside_page {
        ((rest - space::MD) / 2.0).clamp(0.0, PAGE_WIDTH)
    } else {
        rest.clamp(0.0, PAGE_WIDTH)
    };
    let page = egui::Rect::from_min_size(
        egui::pos2(page_left, room.top()),
        egui::vec2(page_width, room.height()),
    );
    framed(ui, "library_shelf", shelf, |ui, height| {
        ui.set_min_height(height);
        shelf::show(ui, local, catalogue, fit, cx);
    });
    if !is_reading || is_reader_beside_page {
        framed(ui, "library_page", page, |ui, height| {
            ui.set_min_height(height);
            open_page(ui, open, Place::Beside, local, cx);
        });
    }
    if is_reading {
        let reader_left = if is_reader_beside_page {
            page.right() + space::MD
        } else {
            page.left()
        };
        let reader = egui::Rect::from_min_max(
            egui::pos2(reader_left, room.top()),
            egui::pos2(room.right().min(reader_left + PAGE_WIDTH), room.bottom()),
        );
        reader::show(ui, reader, local, cx);
    }
}

/// The shelf fills the room, unless the page of the open document fills it, or the page of the
/// open media once "About this media" was pressed. Each sheet keeps the id it has beside the other,
/// so it keeps its scroll position when the window changes its width.
fn one_at_a_time(
    ui: &mut egui::Ui,
    catalogue: &Catalogue,
    open: Option<Opened<'_>>,
    local: &mut Local,
    cx: &mut PanelCx<'_>,
) {
    let room = ui.max_rect();
    let has_page = match &open {
        Some(Opened::Document(..)) => true,
        Some(Opened::Media(media)) => media.title.is_some() && media.title == local.media_page,
        None => false,
    };
    if has_page {
        framed(ui, "library_page", room, |ui, height| {
            ui.set_min_height(height);
            open_page(ui, open, Place::Alone, local, cx);
        });
    } else {
        framed(ui, "library_shelf", room, |ui, height| {
            ui.set_min_height(height);
            shelf::show(ui, local, catalogue, ShelfFit::Alone, cx);
        });
    }
}

fn open_page(
    ui: &mut egui::Ui,
    open: Option<Opened<'_>>,
    place: Place,
    local: &mut Local,
    cx: &mut PanelCx<'_>,
) {
    if place == Place::Alone
        && let Some(label) = back_label(open.as_ref())
    {
        back_row(ui, label, cx.intents);
        ui.add_space(space::SM);
    }
    if cx.shared.ingest.is_running() {
        ui.label(TextRole::Small.rich(ui, INGEST_RUNNING));
        ui.add_space(space::SM);
    }
    match open {
        Some(Opened::Media(media)) => media::show(ui, media, local, cx),
        Some(Opened::Document(media, document)) => {
            // Alone, the back row already names the media.
            let named = (place == Place::Beside).then_some(media);
            document::show(ui, named, document, local, cx);
        }
        None => {
            Placeholder::empty(Icon::LIBRARY, NOTHING_OPEN)
                .hint(NOTHING_OPEN_HINT)
                .show(ui);
        }
    }
}

/// Where Back leads from a page that fills the room: a document to its media, a media to the shelf.
fn back_label<'a>(open: Option<&Opened<'a>>) -> Option<&'a str> {
    match open? {
        Opened::Media(_) => Some(TITLE),
        Opened::Document(media, _) => Some(media.title.as_deref().unwrap_or(NO_MEDIA)),
    }
}

/// The first row of a page that fills the room.
fn back_row(ui: &mut egui::Ui, label: &str, intents: &mut Vec<Intent>) {
    let row = ListRow::new(label)
        .glyph(Icon::CARET_LEFT)
        .size(ControlSize::Large);
    if ui.add(row).clicked() {
        intents.push(Intent::Back);
    }
}

/// Whether the room left in this ui is too narrow for a row of buttons. Call it where the whole
/// width of the page is left, not inside a part that has less, such as the end of a title line.
fn width(ui: &egui::Ui) -> PanelWidth {
    PanelWidth::of(ui, NARROW_BELOW)
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
