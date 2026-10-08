//! The answer to the question, with its citations, and the results by kind.

mod blocks;
mod cards;
mod header;
mod heights;
mod listing;
mod pane;
mod phase;
mod results;
mod states;

use eframe::egui;

use self::listing::Listing;
use self::pane::{Pane, View};
use self::phase::{AnswerTab, Phase};
use crate::contract::Loadable;
use crate::panels::PanelCx;
use crate::theme::space;
use crate::widgets;

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
    let mut pane = Pane {
        ask,
        phase: Phase::of(ask),
        listing: listing.as_ref(),
        view,
        cx,
    };
    let margin = egui::Margin::symmetric(space::LG as i8, space::XS as i8);
    widgets::panel_frame().inner_margin(margin).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        header::show(ui, &mut pane);
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
