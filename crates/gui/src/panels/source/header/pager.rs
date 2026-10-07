//! The page pager: its texts, kept until the page or the page asked for changes, and the stepper
//! that shows them.

use eframe::egui;

use crate::contract::DocId;
use crate::state::SourceNav;
use crate::theme::{TextRole, color};
use crate::widgets::{Step, Stepper};

/// Shown where a page number is not known.
const UNKNOWN: &str = "–";
/// A change of the pager's width smaller than this comes from rounding. It is not worth laying
/// the frame out again.
const WIDTH_SLACK: f32 = 1.0;

/// The texts of the pager and how wide it was when it was last drawn. A page that is on its way
/// shows as the page asked for, in a muted colour, and the printed page number waits for it.
#[derive(Debug, Default)]
pub(super) struct PagerText {
    key: Option<(DocId, u32, u32)>,
    stepper: String,
    label: String,
    width: f32,
}

impl PagerText {
    /// How wide the pager was when it was last drawn, in points.
    pub(super) fn width(&self) -> f32 {
        self.width
    }

    /// Draws the pager and returns the turn the person asked for.
    ///
    /// It is laid out from its left in a room as wide as it was when it was last drawn, so it
    /// ends at the right edge of its row. When its width changes, the frame is laid out once
    /// more before it is shown.
    pub(super) fn show(&mut self, ui: &mut egui::Ui, nav: &SourceNav) -> Option<i32> {
        let room = egui::vec2(self.width.min(ui.available_width()), ui.available_height());
        let from_the_left = egui::Layout::left_to_right(egui::Align::Center);
        let pager = self.view(nav);
        let shown = ui.allocate_ui_with_layout(room, from_the_left, |ui| pager.draw(ui));
        let drawn = shown.response.rect.width();
        if (drawn - self.width).abs() > WIDTH_SLACK {
            self.width = drawn;
            ui.ctx()
                .request_discard("the pager of the source panel changed its width");
        }
        shown.inner
    }

    fn view(&mut self, nav: &SourceNav) -> Pager<'_> {
        let Some(page) = nav.page.ready() else {
            return Pager {
                stepper: UNKNOWN,
                label: None,
                is_turning: false,
                can_go_back: false,
                can_go_on: false,
            };
        };
        let wanted = nav
            .target
            .filter(|target| target.doc == page.doc)
            .map_or(page.page, |target| target.page);
        let is_turning = wanted != page.page;
        let key = (page.doc, page.page, wanted);
        if self.key != Some(key) {
            self.key = Some(key);
            self.stepper = if is_turning {
                "…".to_owned()
            } else {
                let printed = page.printed_page.as_deref().unwrap_or(UNKNOWN);
                format!("p. {printed}")
            };
            self.label = format!("page {wanted} of {}", page.page_count);
        }
        Pager {
            stepper: &self.stepper,
            label: Some(&self.label),
            is_turning,
            can_go_back: wanted > 1,
            can_go_on: wanted < page.page_count,
        }
    }
}

/// What the pager shows in one frame.
#[derive(Debug, Clone, Copy)]
struct Pager<'a> {
    stepper: &'a str,
    /// The place of the page in the chapter, such as "page 5 of 7".
    label: Option<&'a str>,
    /// Another page of the chapter is on its way, so the label is muted.
    is_turning: bool,
    can_go_back: bool,
    can_go_on: bool,
}

impl Pager<'_> {
    fn draw(&self, ui: &mut egui::Ui) -> Option<i32> {
        let step = Stepper::new(self.stepper)
            .previous("Previous page", self.can_go_back)
            .next("Next page", self.can_go_on)
            .show(ui);
        if let Some(label) = self.label {
            let ink = if self.is_turning {
                color::TEXT_MUTED
            } else {
                color::TEXT_SECONDARY
            };
            ui.label(TextRole::Small.rich(label).color(ink));
        }
        step.map(|step| match step {
            Step::Previous => -1,
            Step::Next => 1,
        })
    }
}
