//! The Library page: the stored media with their documents, and what a person can do with each
//! document.

mod document;
mod edit;
mod media;

use eframe::egui;

use crate::contract::{Catalogue, Intent, Loadable};
use crate::panels::PanelCx;
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
const INGEST_RUNNING: &str = "An ingest is running. Tags can be changed when it is done.";

/// One form is open at a time, so opening a second one drops what was typed in the first.
#[derive(Debug, Default)]
pub struct Local {
    editing: Option<edit::Draft>,
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    edit::follow(&mut local.editing, &cx.shared.library);
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
