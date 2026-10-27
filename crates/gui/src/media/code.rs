//! The block that shows a code listing as it was printed: fixed-width type on a well, with the
//! language above it. It is laid out on the window's thread and is not kept.

use eframe::egui::{
    Align, Layout, ScrollArea, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, util, vec2,
};

use super::content_salt;
use super::rich_text::{Clicked, copy_is_chosen};
use crate::theme::{self, TextRole, radius, space};

const PADDING: f32 = space::SM;

/// The room that a block of that many lines takes with its language above it, for a list that
/// cuts the block at a fixed height.
pub const fn height_of(lines: u32) -> f32 {
    TextRole::Small.line_height()
        + space::XS
        + lines as f32 * TextRole::Mono.line_height()
        + 2.0 * PADDING
}

/// Every line is drawn as it is, and a line wider than the room scrolls. It only ever returns
/// `CopyText`, because the text of the app is not selectable and every copy goes through
/// `Intent::CopyText`.
#[must_use = "a copy request is lost"]
pub fn show(ui: &mut Ui, code: &str, language: Option<&str>) -> Option<Clicked> {
    ui.scope(|ui| {
        // The gap is set here, not by the caller, so that `height_of` holds wherever it is drawn.
        ui.spacing_mut().item_spacing.y = space::XS;
        if let Some(language) = language {
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                let language_color = theme::palette(ui.ctx()).text_muted;
                ui.label(TextRole::Small.rich(ui, language).color(language_color));
            });
        }
        listing(ui, code)
    })
    .inner
}

fn listing(ui: &mut Ui, code: &str) -> Option<Clicked> {
    let palette = theme::palette(ui.ctx());
    let galley = TextRole::Mono.galley(ui, code, palette.text);
    let salt = content_salt(ui, util::hash(code));
    let room = vec2(ui.available_width(), galley.size().y + 2.0 * PADDING);
    let (outer, response) = ui.allocate_exact_size(room, Sense::click());
    // SMELL: a result card that the pointer rests on takes this same fill, so in that card the
    // shaded bed of the listing does not show. The block cannot tell what it is drawn on.
    ui.painter().rect_filled(outer, radius::SM, palette.well);
    let mut inside = ui.new_child(UiBuilder::new().max_rect(outer.shrink(PADDING)));
    ScrollArea::horizontal()
        .id_salt(salt)
        .show(&mut inside, |ui| {
            let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
            ui.painter().galley(rect.min, galley, palette.text);
        });
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, code));
    copy_is_chosen(&response, "Copy code").then(|| Clicked::CopyText(code.to_owned()))
}
