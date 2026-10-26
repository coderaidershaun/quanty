//! The bar along the foot of a phone that leads to each view: an icon over a word for every view,
//! and the open one marked.

use eframe::egui::{self, Rect, Sense, pos2, vec2};

use super::tabs::{Tab, tab_tint};
use crate::theme::{Icon, TextRole, color, hairline, radius, size, space, stroke};
use crate::widgets::look::{Look, focus_ring};

/// One place the bar leads to. The label is the accessible name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavItem<'a> {
    pub icon: Icon,
    pub label: &'a str,
}

pub struct NavBar<'a> {
    items: &'a [NavItem<'a>],
    active: usize,
}

impl<'a> NavBar<'a> {
    pub fn new(items: &'a [NavItem<'a>], active: usize) -> Self {
        NavBar { items, active }
    }

    /// Fills the room it is given with one equal cell for each item, and returns the index of the
    /// item the person chose. Never the active one.
    pub fn show(self, ui: &mut egui::Ui) -> Option<usize> {
        let (_, bar) = ui.allocate_space(ui.available_size());
        let painter = ui.painter();
        painter.rect_filled(bar, 0.0, color::DESK);
        let line = hairline(painter, color::HAIRLINE);
        painter.hline(bar.x_range(), bar.top() + line.width / 2.0, line);

        let cell_width = bar.width() / self.items.len().max(1) as f32;
        let mut chosen = None;
        for (index, item) in self.items.iter().enumerate() {
            let left = bar.left() + index as f32 * cell_width;
            let cell = Rect::from_min_size(pos2(left, bar.top()), vec2(cell_width, bar.height()));
            if self.show_item(ui, index, item, cell) {
                chosen = Some(index);
            }
        }
        chosen
    }

    fn show_item(&self, ui: &mut egui::Ui, index: usize, item: &NavItem<'_>, cell: Rect) -> bool {
        let is_active = index == self.active;
        let response = ui.interact(cell, ui.id().with(("nav_item", index)), Sense::click());
        crate::widgets::interactive_cursor(ui, &response);
        Tab::new(item.label).announce(ui, &response, is_active);
        if ui.is_rect_visible(cell) {
            let look = Look::of(&response);
            let tint = tab_tint(is_active, look);
            let galley = TextRole::Small.galley(ui, item.label, tint);
            let pair_height = size::ICON_LG + space::XXS + galley.size().y;
            let top = cell.center().y - pair_height / 2.0;
            let painter = ui.painter();
            let icon_centre = pos2(cell.center().x, top + size::ICON_LG / 2.0);
            item.icon.paint(painter, icon_centre, size::ICON_LG, tint);
            let label_top = top + size::ICON_LG + space::XXS;
            let label_left = cell.center().x - galley.size().x / 2.0;
            painter.galley(pos2(label_left, label_top), galley, tint);
            if is_active {
                let mut mark = cell;
                mark.max.y = cell.top() + stroke::UNDERLINE;
                painter.rect_filled(mark, 0.0, color::RULE);
            }
            if look.focused {
                focus_ring(painter, cell, radius::SM);
            }
        }
        response.clicked() && !is_active
    }
}
