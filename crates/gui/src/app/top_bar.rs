//! The top bar: the logo, the strip of tabs and the model choices, each placed by the form of the
//! window so that the three never overlap and no tab is cut.

use eframe::egui::{self, Rect, pos2, vec2};

use super::layout::Form;
use super::model_picker::{DROPDOWNS_WIDTH, MODEL_BUTTON_WIDTH, Place};
use super::region::region;
use crate::panels::PanelCx;
use crate::theme::{color, hairline, size, space};
use crate::widgets::{self, TabStrip};
use contract::{self, Intent};

const TABS_WIDTH: f32 = 300.0;
const LOGO_WIDTH: f32 = 120.0;
// SMELL: the layout of the panels and the footer keep their own copies of these gutters, so a
// change to the edge of the window must be made in each of them. One shared constant would end it.
const GUTTER: f32 = space::LG;
const GAP: f32 = space::MD;
/// A compact window is narrow, so it spends less on the space at the edges of the bar.
const COMPACT_GUTTER: f32 = space::SM;

/// Where each part of the top bar stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Places {
    pub(super) bar: Rect,
    /// `None` when the logo does not fit beside the tabs and the model choices.
    pub(super) logo: Option<Rect>,
    pub(super) tabs: Rect,
    pub(super) models: Place,
}

/// A wide window keeps its parts where they always stood: the logo at the left, the tabs in the
/// middle and the two dropdowns at the right. A smaller window gives the tabs the room first: the
/// logo shows only when it fits, and the dropdowns become one button unless a medium window has
/// the room for them.
pub(super) fn places(bar: Rect, form: Form) -> Places {
    let gutter = if form == Form::Compact {
        COMPACT_GUTTER
    } else {
        GUTTER
    };
    let left = bar.left() + gutter;
    let right = bar.right() - gutter;
    let centre = bar.center();
    let control_top = centre.y - size::CONTROL_MD / 2.0;
    let dropdowns = Rect::from_min_max(
        pos2(right - DROPDOWNS_WIDTH, bar.top()),
        pos2(right, bar.bottom()),
    );
    let button = Rect::from_min_max(
        pos2(right - MODEL_BUTTON_WIDTH, control_top),
        pos2(right, control_top + size::CONTROL_MD),
    );
    let row_fits = |models_left: f32| left + LOGO_WIDTH + GAP + TABS_WIDTH + GAP <= models_left;

    let models = match form {
        Form::Wide => Place::Inline(dropdowns),
        Form::Medium if row_fits(dropdowns.left()) => Place::Inline(dropdowns),
        Form::Medium | Form::Compact => Place::Button(button),
    };
    let models_left = models.rect().left();
    let logo = row_fits(models_left)
        .then(|| Rect::from_min_size(pos2(left, control_top), vec2(LOGO_WIDTH, size::CONTROL_MD)));
    let start = logo.map_or(left, |logo| logo.right() + GAP);
    let centred = Rect::from_center_size(centre, vec2(TABS_WIDTH, size::TAB.min(bar.height())));
    let tabs = if centred.left() >= start && centred.right() <= models_left - GAP {
        centred
    } else {
        Rect::from_min_max(
            pos2(start, centred.top()),
            pos2(
                (start + TABS_WIDTH).min(models_left - GAP),
                centred.bottom(),
            ),
        )
    };
    Places {
        bar,
        logo,
        tabs,
        models,
    }
}

fn label(tab: contract::Tab) -> &'static str {
    match tab {
        contract::Tab::Ask => "Ask",
        contract::Tab::Library => "Library",
        contract::Tab::Ingest => "Ingest",
        contract::Tab::Datasets => "Datasets",
    }
}

pub(super) fn draw(ui: &mut egui::Ui, places: &Places, cx: &mut PanelCx<'_>) {
    // The line runs across the whole window at the foot of the tab row, not of the bar, so the mark
    // of the open tab sits on it. It is drawn first, so that mark covers it.
    let painter = ui.painter();
    let line = hairline(painter, color::HAIRLINE);
    painter.hline(
        places.bar.x_range(),
        places.tabs.bottom() - line.width / 2.0,
        line,
    );
    if let Some(logo) = places.logo {
        region(ui, "logo", logo, |ui| {
            widgets::logo(ui);
        });
    }
    region(ui, "tabs", places.tabs, |ui| tabs(ui, cx));
}

fn tabs(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    let tabs: Vec<widgets::Tab<'_>> = contract::Tab::ALL
        .iter()
        .map(|tab| widgets::Tab::new(label(*tab)))
        .collect();
    let active = contract::Tab::ALL
        .iter()
        .position(|tab| *tab == cx.shared.tab)
        .unwrap_or_default();
    if let Some(index) = TabStrip::new(&tabs, active).show(ui) {
        cx.intents.push(Intent::OpenTab(contract::Tab::ALL[index]));
    }
}
