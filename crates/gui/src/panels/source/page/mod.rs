//! The Page tab: the page picture with its zoom bar, or the text of a page that has no picture.

mod canvas;
mod text;
mod view;
mod zoom_bar;

use eframe::egui;

use super::row;
use super::states::whole_area;
use super::tabs::TabCx;
use crate::contract::{ImageRef, PagePiece, PageView, PieceKind};
use crate::media::Media;
use crate::media::images::{self, ImageState, Picture};
use crate::theme::{size, space};
use crate::widgets::Placeholder;
use canvas::{Figure, Shown};
use view::{View, Viewport};
use zoom_bar::Percent;

pub(super) const PICTURE_ALT: &str = "Page picture";

#[derive(Debug, Default)]
pub(super) struct State {
    view: View,
    /// The picture on screen, and the number of the page it belongs to.
    on_screen: Option<(ImageRef, u32)>,
    percent: Percent,
}

impl State {
    /// Another page starts at its top and keeps its zoom and its sideways position.
    fn arrive(&mut self, image: &ImageRef, page: u32) {
        if self
            .on_screen
            .as_ref()
            .is_some_and(|(_, shown)| *shown == page)
        {
            return;
        }
        self.view.pan.y = 0.0;
        self.on_screen = Some((image.clone(), page));
    }

    fn old_picture(&self, media: &mut Media) -> Option<(Picture, u32)> {
        let (old, number) = self.on_screen.as_ref()?;
        match media.images.get(old) {
            ImageState::Ready(picture) => Some((picture, *number)),
            ImageState::Loading | ImageState::Failed(_) => None,
        }
    }
}

pub(super) fn prefetch(media: &mut Media, page: &PageView) {
    let likely = [&page.previous_image, &page.next_image, &page.image];
    for image in likely.into_iter().flatten() {
        media.images.prefetch(image);
    }
}

pub(super) fn show(ui: &mut egui::Ui, state: &mut State, cx: &mut TabCx<'_>) {
    let page = cx.page;
    let Some(image) = &page.image else {
        state.on_screen = None;
        text::show(ui, cx);
        return;
    };
    match cx.media.images.get(image) {
        ImageState::Ready(picture) => {
            state.arrive(image, page.page);
            let figure = figure_to_frame(cx.target_piece);
            let is_pending = *cx.reveal == cx.target_piece.map(|piece| piece.number);
            let shown = Shown {
                picture,
                page_number: page.page,
                figure,
                should_reveal: figure.is_some() && is_pending,
            };
            draw(ui, state, &shown);
            *cx.reveal = None;
        }
        ImageState::Loading => match state.old_picture(cx.media) {
            Some((picture, page_number)) => {
                let shown = Shown {
                    picture,
                    page_number,
                    figure: None,
                    should_reveal: false,
                };
                draw(ui, state, &shown);
            }
            None => {
                whole_area(ui, Placeholder::loading("Opening the page"));
            }
        },
        ImageState::Failed(_) => {
            state.on_screen = None;
            let room = ui.available_size();
            images::show(ui, cx.media, image, PICTURE_ALT, room);
        }
    }
}

fn figure_to_frame(piece: Option<&PagePiece>) -> Option<Figure<'_>> {
    let piece = piece.filter(|piece| piece.kind == PieceKind::Figure)?;
    Some(Figure {
        cut: piece.cut?,
        name: piece.label.as_deref().unwrap_or("Figure"),
        caption: piece.caption.as_deref(),
    })
}

fn room_for_bar(body: egui::Rect) -> (egui::Rect, egui::Rect) {
    let bar = egui::Rect::from_min_max(
        egui::pos2(body.left(), body.bottom() - size::CONTROL_SM),
        body.max,
    );
    let area = egui::Rect::from_min_max(
        body.min,
        egui::pos2(body.right(), (bar.top() - space::SM).max(body.top())),
    );
    (area, bar)
}

/// The bar is drawn first, so what it asks for shows in the frame that it is pressed in.
fn draw(ui: &mut egui::Ui, state: &mut State, shown: &Shown<'_>) {
    let (area, bar) = room_for_bar(ui.available_rect_before_wrap());
    let viewport = Viewport {
        rect: area,
        picture: shown.picture.size(),
    };
    let action = row::show_at(ui, bar, |ui| {
        zoom_bar::show(ui, state.view.zoom, &mut state.percent)
    });
    if let Some(action) = action {
        state.view = action.apply(&viewport, state.view);
    }
    let area_ui = egui::UiBuilder::new().max_rect(area);
    ui.scope_builder(area_ui, |ui| canvas::show(ui, &mut state.view, shown));
}
