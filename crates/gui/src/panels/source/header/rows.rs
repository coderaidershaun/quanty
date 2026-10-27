//! Which arrangement the header of the source panel takes in the room it has, and so how much
//! height it leaves for the page.

use eframe::egui::Vec2;

use crate::panels::PanelWidth;
use crate::widgets::{ControlSize, TabStrip};

/// The panel's inner height from which a header has three or four rows. Below it the header is
/// folded, so the page keeps its room.
const FOLD_BELOW: f32 = 440.0;
/// A two-row header is kept only while the page under it keeps this much height.
const PAGE_LEAST: f32 = 64.0;
/// A picker in a header of one row is never narrower than this.
const PICKER_LEAST: f32 = 160.0;

/// How the header is laid out. Its parts are the title with its buttons, the media picker, the
/// document picker and the pager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Rows {
    /// Every part in one row.
    One,
    /// The title with its buttons and the pager, then the two pickers side by side.
    Two,
    /// The title with its buttons, the media picker, then the document picker beside the pager.
    Three,
    /// A narrow panel has no room for two controls side by side, so each row holds one part.
    Four,
    /// A short narrow panel: the title, its buttons and a button that shows the two pickers, then
    /// the pager.
    Folded,
}

/// What the header's parts measure, in points, so that the arrangement is picked before anything
/// is drawn.
#[derive(Debug, Clone, Copy)]
pub(super) struct Parts {
    /// The room inside the panel.
    pub(super) room: Vec2,
    /// The space between two parts.
    pub(super) gap: Vec2,
    pub(super) size: ControlSize,
    pub(super) title: f32,
    pub(super) pager: f32,
    /// The buttons at the right end of the title row.
    pub(super) actions: f32,
}

impl Rows {
    /// A tall panel stacks the parts. A short one saves height: a roomy one takes two rows if they
    /// fit, else one row, else three, and a narrow one folds the pickers away.
    pub(super) fn pick(width: PanelWidth, parts: &Parts) -> Rows {
        let is_short = parts.room.y < FOLD_BELOW;
        match (width, is_short) {
            (PanelWidth::Narrow, false) => Rows::Four,
            (PanelWidth::Narrow, true) => Rows::Folded,
            (PanelWidth::Roomy, false) => Rows::Three,
            (PanelWidth::Roomy, true) if parts.fits_two_rows() => Rows::Two,
            (PanelWidth::Roomy, true) if parts.fits_one_row() => Rows::One,
            (PanelWidth::Roomy, true) => Rows::Three,
        }
    }
}

impl Parts {
    /// The title row holds the title, the pager and the buttons, and the page keeps its least
    /// height under the two rows, the strip of tabs and the zoom bar.
    fn fits_two_rows(&self) -> bool {
        let title_row = self.title + self.pager + self.actions + 2.0 * self.gap.x;
        let header = 2.0 * self.size.height();
        let below = TabStrip::COMPACT_HEIGHT + self.size.height();
        let kept = self.room.y - header - below - 4.0 * self.gap.y;
        title_row <= self.room.x && kept >= PAGE_LEAST
    }

    /// The title, two pickers of their least width, the pager and the buttons stand in one row.
    fn fits_one_row(&self) -> bool {
        let pickers = 2.0 * PICKER_LEAST;
        self.title + pickers + self.pager + self.actions + 4.0 * self.gap.x <= self.room.x
    }
}
