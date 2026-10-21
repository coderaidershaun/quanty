//! Pages: what is drawn is gathered as items in chunks, the chunks are stacked down the pages, and
//! each item is then drawn with the PDF writer.

use krilla::color::luma;
use krilla::geom::{PathBuilder, Point, Rect, Size, Transform};
use krilla::image::Image;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;

use super::faces::{Faces, Run, Typeface};
use super::{CONTENT_HEIGHT, MARGIN, PAGE_HEIGHT, PAGE_WIDTH};

/// Dark grey for text, lighter grey for what stands second, rules and the bed of code.
pub(super) const INK_TEXT: u8 = 28;
pub(super) const INK_SECONDARY: u8 = 102;
pub(super) const INK_RULE: u8 = 64;
pub(super) const INK_BED: u8 = 244;

const NUMBER_SIZE: f32 = 8.0;
const NUMBER_FROM_BOTTOM: f32 = 36.0;

/// Something drawn on a page. The place is in points from the left margin and from the top of
/// the chunk it belongs to; the page puts it where its chunk lands.
pub(super) enum Item {
    /// A run of text with its baseline at `y`.
    Glyphs {
        x: f32,
        y: f32,
        run: Run,
        size: f32,
        ink: u8,
    },
    Picture {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        image: Image,
    },
    Rule {
        x1: f32,
        x2: f32,
        y: f32,
        thickness: f32,
        ink: u8,
    },
    Bed {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        ink: u8,
    },
}

impl Item {
    fn moved(mut self, by: f32) -> Item {
        match &mut self {
            Item::Glyphs { y, .. }
            | Item::Picture { y, .. }
            | Item::Rule { y, .. }
            | Item::Bed { y, .. } => *y += by,
        }
        self
    }
}

/// The smallest part that is never split between two pages.
pub(super) struct Chunk {
    pub(super) height: f32,
    /// Space wanted above it and below it. Of two neighbours, the larger wish is the gap.
    pub(super) before: f32,
    pub(super) after: f32,
    /// How many of the chunks after this one must stand on the same page with it.
    pub(super) keep_with_next: usize,
    pub(super) items: Vec<Item>,
}

impl Chunk {
    pub(super) fn new(height: f32, items: Vec<Item>) -> Chunk {
        Chunk {
            height,
            before: 0.0,
            after: 0.0,
            keep_with_next: 0,
            items,
        }
    }
}

/// Stacks the chunks down pages. A chunk that does not fit goes to the next page, with the chunks
/// that must stay with it. The items come back placed from the top margin of their page.
pub(super) fn paginate(mut chunks: Vec<Chunk>) -> Vec<Vec<Item>> {
    let mut pages: Vec<Vec<Item>> = Vec::new();
    let mut page: Vec<Item> = Vec::new();
    let mut used = 0.0_f32;
    let mut previous_after = 0.0_f32;
    for at in 0..chunks.len() {
        let page_is_empty = page.is_empty();
        let mut gap = if page_is_empty {
            0.0
        } else {
            previous_after.max(chunks[at].before)
        };
        let last_kept = (at + chunks[at].keep_with_next).min(chunks.len() - 1);
        let mut needed = chunks[at].height;
        for kept in at + 1..=last_kept {
            needed += chunks[kept - 1].after.max(chunks[kept].before) + chunks[kept].height;
        }
        if !page_is_empty && used + gap + needed > CONTENT_HEIGHT {
            pages.push(std::mem::take(&mut page));
            used = 0.0;
            gap = 0.0;
        }
        let top = used + gap;
        let items = std::mem::take(&mut chunks[at].items);
        page.extend(items.into_iter().map(|item| item.moved(top)));
        used = top + chunks[at].height;
        previous_after = chunks[at].after;
    }
    pages.push(page);
    pages
}

/// Draws every page, with its number at the foot.
pub(super) fn draw(pdf: &mut krilla::Document, faces: &Faces, pages: Vec<Vec<Item>>) {
    let a4 = PageSettings::from_wh(PAGE_WIDTH, PAGE_HEIGHT)
        .expect("the width and the height of an A4 page are more than zero");
    let total = pages.len();
    for (index, items) in pages.into_iter().enumerate() {
        let mut page = pdf.start_page_with(a4.clone());
        let mut surface = page.surface();
        for item in &items {
            draw_item(&mut surface, item);
        }
        let number = format!("{} / {total}", index + 1);
        let runs = faces.shape(&number, Typeface::Serif);
        let width: f32 = runs.iter().map(|run| run.width(NUMBER_SIZE)).sum();
        let mut x = (PAGE_WIDTH - width) / 2.0;
        for run in &runs {
            draw_run(
                &mut surface,
                x,
                PAGE_HEIGHT - NUMBER_FROM_BOTTOM,
                run,
                NUMBER_SIZE,
                INK_SECONDARY,
            );
            x += run.width(NUMBER_SIZE);
        }
        surface.finish();
        page.finish();
    }
}

fn draw_item(surface: &mut Surface<'_>, item: &Item) {
    match item {
        Item::Glyphs {
            x,
            y,
            run,
            size,
            ink,
        } => draw_run(surface, MARGIN + x, MARGIN + y, run, *size, *ink),
        Item::Picture {
            x,
            y,
            width,
            height,
            image,
        } => {
            if let Some(size) = Size::from_wh(*width, *height) {
                surface.push_transform(&Transform::from_translate(MARGIN + x, MARGIN + y));
                surface.draw_image(image.clone(), size);
                surface.pop();
            }
        }
        Item::Rule {
            x1,
            x2,
            y,
            thickness,
            ink,
        } => {
            let mut path = PathBuilder::new();
            path.move_to(MARGIN + x1, MARGIN + y);
            path.line_to(MARGIN + x2, MARGIN + y);
            if let Some(path) = path.finish() {
                surface.set_fill(None);
                surface.set_stroke(Some(Stroke {
                    paint: luma::Color::new(*ink).into(),
                    width: *thickness,
                    ..Stroke::default()
                }));
                surface.draw_path(&path);
                surface.set_stroke(None);
            }
        }
        Item::Bed {
            x,
            y,
            width,
            height,
            ink,
        } => {
            let mut path = PathBuilder::new();
            if let Some(rect) = Rect::from_xywh(MARGIN + x, MARGIN + y, *width, *height) {
                path.push_rect(rect);
            }
            if let Some(path) = path.finish() {
                surface.set_fill(Some(fill(*ink)));
                surface.set_stroke(None);
                surface.draw_path(&path);
            }
        }
    }
}

fn draw_run(surface: &mut Surface<'_>, x: f32, y: f32, run: &Run, size: f32, ink: u8) {
    surface.set_fill(Some(fill(ink)));
    surface.draw_glyphs(
        Point::from_xy(x, y),
        &run.glyphs,
        run.font.clone(),
        &run.text,
        size,
        false,
    );
}

fn fill(ink: u8) -> Fill {
    Fill {
        paint: luma::Color::new(ink).into(),
        ..Fill::default()
    }
}
