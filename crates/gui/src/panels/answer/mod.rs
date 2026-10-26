//! The answer to the question, with its citations, and the results by kind.

mod blocks;
mod cards;
mod header;
mod listing;
mod pane;
mod phase;
mod results;
mod share;
mod states;

use eframe::egui;

use self::listing::Listing;
use self::pane::{Pane, View};
use self::phase::{AnswerTab, Phase};
use crate::panels::{PanelCx, exporting};
use crate::theme::space;
use crate::widgets;
use contract::{ExportPlace, Loadable};

/// A pane whose content is narrower than this counts as narrow, as on a phone. A phone's pane has
/// 340 points of content and the narrowest wide window 682, so a wide window never does.
const NARROW: f32 = 480.0;

#[derive(Debug, Default)]
pub struct Local {
    seen: u64,
    listing: Option<Listing>,
    view: View,
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let shared = cx.shared;
    let ask = &shared.ask;
    if local.seen != ask.generation {
        *local = Local {
            seen: ask.generation,
            ..Local::default()
        };
    }
    if local.listing.is_none()
        && let Loadable::Ready(reply) = &ask.search
    {
        local.listing = Some(Listing::of(&reply.results));
    }
    let Local { listing, view, .. } = local;
    let margin = egui::Margin::symmetric(space::LG as i8, space::XS as i8);
    widgets::panel_frame(widgets::Surface::Sheet)
        .inner_margin(margin)
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            let mut pane = Pane {
                ask,
                phase: Phase::of(ask),
                listing: listing.as_ref(),
                view,
                is_narrow: ui.available_width() < NARROW,
                cx,
            };
            header::show(ui, &mut pane);
            let place = ExportPlace::Answer {
                generation: ask.generation,
            };
            exporting::strip(ui, pane.cx, place);
            body(ui, &mut pane);
        });
}

fn body(ui: &mut egui::Ui, pane: &mut Pane<'_, '_>) {
    match pane.phase {
        Phase::Found { written } => match pane.active() {
            AnswerTab::Answer => blocks::show(ui, pane, written),
            tab => results::list(ui, pane, tab),
        },
        _ => states::whole(ui, pane),
    }
}
