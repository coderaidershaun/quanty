//! The widget that draws a picture in a box: the picture itself, a spinner while it loads, or a
//! card that says why it failed and offers to try again.

use eframe::egui;

use super::picture::{ImageFailure, ImageState, Picture};
use crate::contract::ImageRef;
use crate::media::Media;
use crate::theme::{Icon, color, radius, size};
use crate::widgets::{Placeholder, spinner};

/// Under this height the failure card does not fit, so an icon stands in for it.
// SMELL: this number belongs to the failure card, which does not give out its least height. If
// the card grows, this must follow by hand.
const CARD_LEAST_HEIGHT: f32 = 120.0;

/// A picture fitted inside `max_size`, with its loading and failed states drawn. Senses
/// clicks. The picture is an `Image` node named `alt`; while loading there is a spinner named
/// `Loading picture: {alt}`; a failure says `Picture not found` or `Picture cannot be read` and
/// has a `Retry` button. The loading and failed states fill `max_size`, so it must be finite.
pub fn show(
    ui: &mut egui::Ui,
    media: &mut Media,
    image: &ImageRef,
    alt: &str,
    max_size: egui::Vec2,
) -> egui::Response {
    let room = egui::vec2(max_size.x.min(ui.available_width()), max_size.y);
    match media.images.get(image) {
        ImageState::Ready(picture) => ready(ui, picture, alt, room),
        ImageState::Loading => waiting(ui, alt, room),
        ImageState::Failed(failure) => {
            let (response, retry_clicked) = failed(ui, image, failure, room);
            if retry_clicked {
                media.images.retry(image);
            }
            response
        }
    }
}

fn ready(ui: &mut egui::Ui, picture: Picture, alt: &str, room: egui::Vec2) -> egui::Response {
    let own = picture.size();
    let shown = own * (room.x / own.x).min(room.y / own.y).min(1.0);
    let centred = egui::Layout::top_down(egui::Align::Center);
    ui.allocate_ui_with_layout(egui::vec2(room.x, shown.y), centred, |ui| {
        let (rect, response) = ui.allocate_exact_size(shown, egui::Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, alt));
        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, 0.0, color::PAGE);
            picture.paint(ui.painter(), rect, egui::Color32::WHITE);
        }
        response
    })
    .inner
}

fn waiting(ui: &mut egui::Ui, alt: &str, room: egui::Vec2) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(room, egui::Sense::click());
    ui.painter().rect_filled(rect, radius::MD, color::RAISED);
    let middle = egui::Layout::centered_and_justified(egui::Direction::TopDown);
    let mut inside = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(middle));
    spinner(&mut inside, &format!("Loading picture: {alt}"));
    response
}

/// The box with the reason, and whether its `Retry` button was clicked.
fn failed(
    ui: &mut egui::Ui,
    image: &ImageRef,
    failure: ImageFailure,
    room: egui::Vec2,
) -> (egui::Response, bool) {
    let title = match failure {
        ImageFailure::Missing => "Picture not found",
        ImageFailure::Unreadable => "Picture cannot be read",
    };
    let (rect, response) = ui.allocate_exact_size(room, egui::Sense::click());
    ui.painter().rect_filled(rect, radius::MD, color::RAISED);
    if room.y < CARD_LEAST_HEIGHT {
        // A box this small has no room for the Retry button. The failure stays until a caller
        // forgets it, or the picture is shown in a bigger box and Retry is clicked there.
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            Icon::BROKEN_IMAGE.glyph(),
            Icon::font(size::ICON_LG),
            color::TEXT_MUTED,
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title));
        return (response.on_hover_text(title), false);
    }
    let centred = egui::Layout::top_down(egui::Align::Center);
    let mut inside = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(centred));
    inside.set_clip_rect(rect.intersect(ui.clip_rect()));
    let card = Placeholder::error(title)
        .hint(&hint(image, failure))
        .action("Retry")
        .show(&mut inside);
    (response, card.action_clicked)
}

fn hint(image: &ImageRef, failure: ImageFailure) -> String {
    let name = image.path.file_name().map_or_else(
        || image.path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    match failure {
        ImageFailure::Missing => format!(
            "{name} is not in the content folder. Put the folder back or ingest the document again."
        ),
        ImageFailure::Unreadable => format!("{name} is not a PNG or JPEG that can be opened."),
    }
}
