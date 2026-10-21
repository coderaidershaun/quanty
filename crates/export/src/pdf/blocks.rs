//! The blocks of a document as chunks of a page: the title, the origin, the blocks that are not
//! Markdown, and the list of sources.

use krilla::image::Image;

use super::TEXT_WIDTH;
use super::faces::Typeface;
use super::flow::{
    Align, BODY, Layout, Line, Look, NOTE, SMALL, formula_picture, keep_lines_together,
};
use super::pages::{Chunk, INK_BED, INK_TEXT, Item};
use super::pieces::{INDENT, ITEM_AFTER, space};
use super::read::{Span, read_inline};
use crate::document::language_word;
use crate::math::Display;
use crate::pictures::{Picture, PictureKind};
use crate::{Block, Document, Source};

const TITLE_AFTER: f32 = 8.0;
const ORIGIN_AFTER: f32 = 16.0;
const LIST_AFTER: f32 = 7.0;
/// A picture is drawn at half its pixels, so it is as sharp as the formulas around it, and it
/// takes at most this share of the height of a page.
const PICTURE_SCALE: f32 = 0.5;
const PICTURE_MAX_SHARE: f32 = 0.45;
const CAPTION: Look = SMALL.italic();
/// The space between a picture and its caption.
const CAPTION_GAP: f32 = 6.0;
const DISPLAY_FORMULA_EM: f32 = 11.0 * 1.25;
/// The least room between a formula and its label.
const LABEL_GAP: f32 = 12.0;
/// The room above and below a formula, a figure and a code listing.
const BLOCK_ROOM: f32 = 8.0;
const CODE_SIZE: f32 = 8.5;
const CODE_LEADING: f32 = 11.5;
const CODE_PADDING: f32 = 6.0;
const LANGUAGE_SIZE: f32 = 7.5;
/// The least room between the end of a line of code and the language beside it.
const LANGUAGE_GAP: f32 = 12.0;
/// How far the code moves down when the language needs a row of its own.
const LANGUAGE_ROW: f32 = LANGUAGE_SIZE + 3.0;
const SOURCE_MARK_WIDTH: f32 = 24.0;

impl Layout<'_> {
    pub(super) fn document(&mut self, document: &Document) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut title = self.set(
            &[Span::text(&document.title)],
            Look::new(20.0, 26.0).strong(),
            (0.0, TEXT_WIDTH),
            Align::Left,
        );
        space(&mut title, 0.0, TITLE_AFTER);
        chunks.extend(title);
        if !document.origin.is_empty() {
            let mut origin = self.set(
                &[Span::text(&document.origin)],
                BODY.italic().secondary(),
                (0.0, TEXT_WIDTH),
                Align::Left,
            );
            space(&mut origin, 0.0, ORIGIN_AFTER);
            chunks.extend(origin);
        }
        for block in &document.blocks {
            chunks.extend(self.block(block));
        }
        if !document.sources.is_empty() {
            chunks.extend(self.sources(&document.sources));
        }
        chunks
    }

    fn block(&mut self, block: &Block) -> Vec<Chunk> {
        match block {
            Block::Heading(heading) => self.heading(2, &[Span::text(heading)]),
            Block::Text { markdown, cites } => self.markdown(markdown, cites),
            Block::List(points) => {
                let mut chunks = Vec::new();
                for point in points {
                    let spans = read_inline(point);
                    let mut item = self.hanging("•", &spans, BODY, (0.0, INDENT));
                    space(&mut item, 0.0, ITEM_AFTER);
                    chunks.extend(item);
                }
                space(&mut chunks, 0.0, LIST_AFTER);
                chunks
            }
            Block::Formula {
                latex,
                label,
                caption,
                cite,
            } => self.formula(latex, label.as_deref(), caption_spans(caption, *cite)),
            Block::Figure {
                picture,
                description,
                caption,
                cite,
            } => {
                let caption = caption_spans(caption, *cite);
                let image = picture.as_deref().and_then(Picture::read);
                self.figure(image, description, caption)
            }
            Block::Table {
                markdown,
                caption,
                cite,
            } => {
                let spans = caption_spans(caption, *cite);
                let mut chunks = self.set(&spans, CAPTION, (0.0, TEXT_WIDTH), Align::Center);
                space(&mut chunks, 4.0, 4.0);
                if let Some(last) = chunks.last_mut() {
                    last.keep_with_next = 2;
                }
                chunks.extend(self.markdown(markdown, &[]));
                chunks
            }
            Block::Code {
                code,
                language,
                caption,
                cite,
            } => {
                let language = language_word(language.as_deref());
                self.code(code, Some(language), caption_spans(caption, *cite))
            }
        }
    }

    /// A formula set apart, with the label at the right and the caption under it. A formula that
    /// cannot be typeset is shown as its LaTeX in code type.
    pub(super) fn formula(
        &mut self,
        latex: &str,
        label: Option<&str>,
        caption: Vec<Span>,
    ) -> Vec<Chunk> {
        let label = label.filter(|label| !label.is_empty());
        let label_look = BODY.secondary();
        let label_width = label.map_or(0.0, |label| {
            self.faces.width(label, Typeface::Serif, label_look.size)
        });
        let room = TEXT_WIDTH - label.map_or(0.0, |_| label_width + LABEL_GAP);
        let Some(formula) = self.formulas.get(latex, Display::Block) else {
            let mut chunks = self.set(&[Span::code(latex)], BODY, (0.0, room), Align::Left);
            if let (Some(label), Some(first)) = (label, chunks.first_mut()) {
                let (baseline, _) = self.line_box(BODY);
                let at = (TEXT_WIDTH - label_width, baseline);
                self.draw_text(label, label_look, at, &mut first.items);
            }
            let look = NOTE.italic().secondary();
            chunks.extend(self.set(&caption, look, (0.0, TEXT_WIDTH), Align::Center));
            space(&mut chunks, BLOCK_ROOM, BLOCK_ROOM);
            return chunks;
        };
        let em = DISPLAY_FORMULA_EM;
        let whole_width = formula.picture_width_em() as f32 * em;
        let whole_height = formula.picture_height_em() as f32 * em;
        let scale = (room / whole_width)
            .min(super::CONTENT_HEIGHT * PICTURE_MAX_SHARE / whole_height)
            .min(1.0);
        let em = em * scale;
        let ascent = formula.ascent_em as f32 * em;
        let bleed = formula.bleed_em as f32 * em;
        let baseline = bleed + ascent;
        let width = formula.width_em as f32 * em;
        // Centred in the text width, unless that would reach the label.
        let left = ((TEXT_WIDTH - width) / 2.0).min(room - width - bleed);
        let mut items: Vec<Item> = formula_picture(&formula, em, (left, baseline))
            .into_iter()
            .collect();
        if let Some(label) = label {
            let at = (TEXT_WIDTH - label_width, baseline);
            self.draw_text(label, label_look, at, &mut items);
        }
        let mut height = formula.picture_height_em() as f32 * em;
        let lines = self.caption_lines(&caption, NOTE.italic().secondary());
        if !lines.is_empty() {
            height += 4.0;
            height += self.draw_lines(&lines, (0.0, height, TEXT_WIDTH), Align::Center, &mut items);
        }
        let mut chunk = Chunk::new(height, items);
        chunk.before = BLOCK_ROOM;
        chunk.after = BLOCK_ROOM;
        vec![chunk]
    }

    /// A picture with the caption under it, kept on one page. With no readable picture, the
    /// description stands in for it.
    fn figure(
        &mut self,
        picture: Option<Picture>,
        description: &str,
        caption: Vec<Span>,
    ) -> Vec<Chunk> {
        let look = CAPTION;
        let image = picture.and_then(|picture| {
            let data = picture.bytes.clone().into();
            let image = match picture.kind {
                PictureKind::Png => Image::from_png(data, true),
                PictureKind::Jpeg => Image::from_jpeg(data, true),
            };
            image.ok().map(|image| (image, picture))
        });
        let Some((image, picture)) = image else {
            let mut chunks = if description.is_empty() {
                Vec::new()
            } else {
                self.set(
                    &[Span::text(description)],
                    BODY,
                    (0.0, TEXT_WIDTH),
                    Align::Left,
                )
            };
            let mut caption = self.set(&caption, look, (0.0, TEXT_WIDTH), Align::Center);
            space(&mut caption, 3.0, 0.0);
            chunks.extend(caption);
            space(&mut chunks, BLOCK_ROOM, BLOCK_ROOM);
            return chunks;
        };
        let natural_width = picture.width_px as f32 * PICTURE_SCALE;
        let natural_height = picture.height_px as f32 * PICTURE_SCALE;
        let scale = (TEXT_WIDTH / natural_width)
            .min(super::CONTENT_HEIGHT * PICTURE_MAX_SHARE / natural_height)
            .min(1.0);
        let (width, height) = (natural_width * scale, natural_height * scale);
        let mut items = vec![Item::Picture {
            x: (TEXT_WIDTH - width) / 2.0,
            y: 0.0,
            width,
            height,
            image,
        }];
        let mut total = height;
        let lines = self.caption_lines(&caption, look);
        if !lines.is_empty() {
            total += CAPTION_GAP;
            total += self.draw_lines(&lines, (0.0, total, TEXT_WIDTH), Align::Center, &mut items);
        }
        let mut chunk = Chunk::new(total, items);
        chunk.before = BLOCK_ROOM;
        chunk.after = BLOCK_ROOM;
        vec![chunk]
    }

    fn caption_lines(&mut self, caption: &[Span], look: Look) -> Vec<Line> {
        let atoms = self.atoms(caption, look);
        if atoms.is_empty() {
            return Vec::new();
        }
        self.lines(atoms, TEXT_WIDTH, look)
    }

    /// Code on a grey bed, one line of the bed to a chunk so that a long listing can run over a
    /// page. A line that is wider than the bed is wrapped at a character.
    pub(super) fn code(
        &mut self,
        code: &str,
        language: Option<&str>,
        caption: Vec<Span>,
    ) -> Vec<Chunk> {
        let advance = self.faces.width("0", Typeface::Code, CODE_SIZE);
        let inner_width = TEXT_WIDTH - 2.0 * CODE_PADDING;
        let per_row = ((inner_width / advance) as usize).max(1);
        let rows = wrap_code(code, per_row);
        let (rise, fall) = self.faces.extent(Typeface::Code);
        let baseline = (CODE_LEADING - (rise + fall) * CODE_SIZE) / 2.0 + rise * CODE_SIZE;
        let language = language.filter(|language| !language.is_empty());
        // The language stands at the right end of the first line, or on a row of its own above
        // it when the line would reach it.
        let tag_row = language.map_or(0.0, |language| {
            let tag_width = self.faces.width(language, Typeface::Serif, LANGUAGE_SIZE);
            let first_width = rows
                .first()
                .map_or(0.0, |row| self.faces.width(row, Typeface::Code, CODE_SIZE));
            if first_width + LANGUAGE_GAP + tag_width > inner_width {
                LANGUAGE_ROW
            } else {
                0.0
            }
        });
        let count = rows.len();
        let mut chunks: Vec<Chunk> = Vec::with_capacity(count);
        for (at, row) in rows.iter().enumerate() {
            let top_padding = if at == 0 { CODE_PADDING + tag_row } else { 0.0 };
            let bottom_padding = if at + 1 == count { CODE_PADDING } else { 0.0 };
            let height = top_padding + CODE_LEADING + bottom_padding;
            let mut items = vec![Item::Bed {
                x: 0.0,
                y: 0.0,
                width: TEXT_WIDTH,
                height,
                ink: INK_BED,
            }];
            if at == 0
                && let Some(language) = language
            {
                let tag = BODY.secondary();
                let tag = Look {
                    size: LANGUAGE_SIZE,
                    ..tag
                };
                let width = self.faces.width(language, Typeface::Serif, LANGUAGE_SIZE);
                let at_x = TEXT_WIDTH - CODE_PADDING - width;
                self.draw_text(
                    language,
                    tag,
                    (at_x, CODE_PADDING + LANGUAGE_SIZE),
                    &mut items,
                );
            }
            let mut x = CODE_PADDING;
            for run in self.faces.shape(row, Typeface::Code) {
                let width = run.width(CODE_SIZE);
                items.push(Item::Glyphs {
                    x,
                    y: top_padding + baseline,
                    run,
                    size: CODE_SIZE,
                    ink: INK_TEXT,
                });
                x += width;
            }
            chunks.push(Chunk::new(height, items));
        }
        keep_lines_together(&mut chunks);
        let mut caption_chunks = self.set(&caption, CAPTION, (0.0, TEXT_WIDTH), Align::Center);
        if !caption_chunks.is_empty() {
            space(&mut caption_chunks, 3.0, 0.0);
            if let Some(last) = chunks.last_mut() {
                last.keep_with_next = last.keep_with_next.max(1);
            }
            chunks.extend(caption_chunks);
        }
        space(&mut chunks, 4.0, 8.0);
        chunks
    }

    fn sources(&mut self, sources: &[Source]) -> Vec<Chunk> {
        let mut chunks = self.heading(2, &[Span::text("Sources")]);
        let look = SMALL;
        for source in sources {
            let marker = format!("[{}]", source.number);
            let spans = [Span::text(&source.text)];
            let mut item = self.hanging(&marker, &spans, look, (0.0, SOURCE_MARK_WIDTH));
            space(&mut item, 0.0, ITEM_AFTER);
            chunks.extend(item);
        }
        chunks
    }
}

/// The caption as spans, followed by the mark of the source it is from.
fn caption_spans(caption: &str, cite: Option<usize>) -> Vec<Span> {
    let mut spans = read_inline(caption);
    spans.extend(cite.map(Span::cite));
    spans
}

/// The lines of the code, each cut into rows of at most `per_row` characters.
fn wrap_code(code: &str, per_row: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for line in code.trim_end_matches('\n').split('\n') {
        let line = line.trim_end_matches('\r').replace('\t', "    ");
        let characters: Vec<char> = line.chars().collect();
        if characters.is_empty() {
            rows.push(String::new());
        }
        for row in characters.chunks(per_row) {
            rows.push(row.iter().collect());
        }
    }
    rows
}
