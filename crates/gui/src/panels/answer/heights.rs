//! Remembers how tall each block of a long column was when it was last drawn, so that a block
//! off screen only takes its room and costs nothing else.

use eframe::egui;

#[derive(Debug, Default)]
pub(super) struct Heights {
    estimate: f32,
    known: Vec<Option<f32>>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Slot {
    pub(super) rect: egui::Rect,
    pub(super) is_drawn: bool,
}

impl Heights {
    /// Takes the heights out of the pane for as long as a list draws, so that the closures that
    /// draw its blocks can borrow the rest of the pane. `estimate` is how high a block is taken
    /// to be until it has been drawn once, which only the list that draws it knows.
    pub(super) fn take(heights: &mut Heights, estimate: f32) -> Heights {
        Heights {
            estimate,
            ..std::mem::take(heights)
        }
    }

    /// A block that is drawn and one that is skipped both take one id from the parent, so the
    /// ids of the blocks in view do not move when the list scrolls.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        draw: impl FnOnce(&mut egui::Ui),
    ) -> Slot {
        if self.known.len() <= index {
            self.known.resize(index + 1, None);
        }
        let width = ui.available_width();
        let height = self.known[index].unwrap_or(self.estimate);
        let guess = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(width, height));
        if !ui.is_rect_visible(guess) {
            let (_, rect) = ui.allocate_space(egui::vec2(width, height));
            return Slot {
                rect,
                is_drawn: false,
            };
        }
        let rect = ui.push_id(index, draw).response.rect;
        self.known[index] = Some(rect.height());
        Slot {
            rect,
            is_drawn: true,
        }
    }
}
