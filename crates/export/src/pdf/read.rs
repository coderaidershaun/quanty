//! Reads the Markdown of a block into the pieces a page is made of: paragraphs, headings, list
//! items, code, tables and the rest, with their text as spans that carry no measure yet.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Tag, TagEnd};

use crate::commonmark;

/// One stretch of text in a line, with how it is set.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Span {
    pub(super) kind: SpanKind,
    pub(super) is_emphasised: bool,
    pub(super) is_strong: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum SpanKind {
    Text(String),
    Code(String),
    Math(String),
    /// A mark in the text that points to a note.
    NoteMark(String),
    /// The mark of the source a block cites, such as `[1]`.
    Cite(String),
    LineBreak,
}

impl Span {
    pub(super) fn text(text: &str) -> Span {
        Span::plain(SpanKind::Text(text.to_owned()))
    }

    pub(super) fn code(code: &str) -> Span {
        Span::plain(SpanKind::Code(code.to_owned()))
    }

    pub(super) fn cite(number: usize) -> Span {
        Span::plain(SpanKind::Cite(format!("[{number}]")))
    }

    fn plain(kind: SpanKind) -> Span {
        Span {
            kind,
            is_emphasised: false,
            is_strong: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Table {
    pub(super) alignments: Vec<Alignment>,
    pub(super) header: Vec<Vec<Span>>,
    pub(super) rows: Vec<Vec<Vec<Span>>>,
}

/// A part of a page that is set as a whole. `indent` counts the lists and the quotes it is in.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Piece {
    Paragraph {
        spans: Vec<Span>,
        indent: usize,
        is_quoted: bool,
    },
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Item {
        marker: String,
        spans: Vec<Span>,
        indent: usize,
    },
    Code {
        code: String,
        language: Option<String>,
    },
    Table(Table),
    Formula(String),
    Rule,
    Note {
        label: String,
        spans: Vec<Span>,
    },
}

/// The pieces of `markdown` in order.
pub(super) fn read(markdown: &str) -> Vec<Piece> {
    let mut reader = Reader::default();
    for event in commonmark::parse(markdown) {
        reader.event(event);
    }
    reader.flush_spans();
    reader.pieces
}

#[derive(Default)]
struct Reader {
    pieces: Vec<Piece>,
    spans: Vec<Span>,
    emphasis: usize,
    strong: usize,
    /// One entry for each list that is open: the number of its next item when it is ordered.
    lists: Vec<Option<u64>>,
    quotes: usize,
    marker: Option<String>,
    heading: Option<u8>,
    note: Option<String>,
    code: Option<(String, Option<String>)>,
    table: Option<TableBuilder>,
}

struct TableBuilder {
    alignments: Vec<Alignment>,
    header: Vec<Vec<Span>>,
    rows: Vec<Vec<Vec<Span>>>,
    row: Vec<Vec<Span>>,
}

impl Reader {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                if let Some((code, _)) = &mut self.code {
                    code.push_str(&text);
                } else {
                    self.push(SpanKind::Text(text.into_string()));
                }
            }
            Event::Code(code) => self.push(SpanKind::Code(code.into_string())),
            Event::InlineMath(latex) => self.push(SpanKind::Math(latex.into_string())),
            Event::DisplayMath(latex) => {
                self.flush_spans();
                self.pieces.push(Piece::Formula(latex.into_string()));
            }
            Event::FootnoteReference(label) => self.push(SpanKind::NoteMark(label.into_string())),
            Event::SoftBreak => self.push(SpanKind::Text(" ".to_owned())),
            Event::HardBreak => self.push(SpanKind::LineBreak),
            Event::Rule => {
                self.flush_spans();
                self.pieces.push(Piece::Rule);
            }
            Event::TaskListMarker(_) => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.flush_spans();
                self.heading = Some(match level {
                    HeadingLevel::H1 | HeadingLevel::H2 => 2,
                    _ => 3,
                });
            }
            Tag::BlockQuote(_) => {
                self.flush_spans();
                self.quotes += 1;
            }
            Tag::CodeBlock(kind) => {
                self.flush_spans();
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().map(str::to_owned)
                    }
                    CodeBlockKind::Indented => None,
                };
                self.code = Some((String::new(), language));
            }
            Tag::List(first) => {
                self.flush_spans();
                self.lists.push(first);
            }
            Tag::Item => {
                self.flush_spans();
                self.marker = Some(match self.lists.last_mut() {
                    Some(Some(number)) => {
                        let marker = format!("{number}.");
                        *number += 1;
                        marker
                    }
                    _ => "•".to_owned(),
                });
            }
            Tag::FootnoteDefinition(label) => {
                self.flush_spans();
                self.note = Some(label.into_string());
            }
            Tag::Table(alignments) => {
                self.flush_spans();
                self.table = Some(TableBuilder {
                    alignments,
                    header: Vec::new(),
                    rows: Vec::new(),
                    row: Vec::new(),
                });
            }
            Tag::Emphasis | Tag::Image { .. } => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::Link { .. }
            | Tag::Strikethrough
            | Tag::HtmlBlock
            | Tag::MetadataBlock(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Superscript
            | Tag::Subscript => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Item => self.flush_spans(),
            TagEnd::Heading(_) => {
                let spans = std::mem::take(&mut self.spans);
                if let Some(level) = self.heading.take() {
                    self.pieces.push(Piece::Heading { level, spans });
                }
            }
            TagEnd::BlockQuote(_) => {
                self.flush_spans();
                self.quotes = self.quotes.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                if let Some((code, language)) = self.code.take() {
                    self.pieces.push(Piece::Code { code, language });
                }
            }
            TagEnd::List(_) => {
                self.flush_spans();
                self.lists.pop();
            }
            TagEnd::FootnoteDefinition => {
                self.flush_spans();
                self.note = None;
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.pieces.push(Piece::Table(Table {
                        alignments: table.alignments,
                        header: table.header,
                        rows: table.rows,
                    }));
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = &mut self.table {
                    table.header = std::mem::take(&mut table.row);
                }
            }
            TagEnd::TableRow => {
                if let Some(table) = &mut self.table {
                    let row = std::mem::take(&mut table.row);
                    table.rows.push(row);
                }
            }
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.spans);
                if let Some(table) = &mut self.table {
                    table.row.push(cell);
                }
            }
            TagEnd::Emphasis | TagEnd::Image => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            _ => {}
        }
    }

    fn push(&mut self, kind: SpanKind) {
        self.spans.push(Span {
            kind,
            is_emphasised: self.emphasis > 0,
            is_strong: self.strong > 0,
        });
    }

    /// Closes the text read so far as a paragraph, a list item or a note.
    fn flush_spans(&mut self) {
        if self.table.is_some() || self.heading.is_some() {
            return;
        }
        let spans = std::mem::take(&mut self.spans);
        if spans.is_empty() {
            return;
        }
        let indent = self.lists.len().saturating_sub(1) + self.quotes;
        let piece = if let Some(label) = self.note.clone() {
            Piece::Note { label, spans }
        } else if let Some(marker) = self.marker.take() {
            Piece::Item {
                marker,
                spans,
                indent,
            }
        } else {
            Piece::Paragraph {
                spans,
                indent: indent + usize::from(!self.lists.is_empty()),
                is_quoted: self.quotes > 0,
            }
        };
        self.pieces.push(piece);
    }
}

/// The spans of one line of Markdown, such as a point of a list or a caption. Anything that is
/// more than one line of text is read as the text of all its lines.
pub(super) fn read_inline(markdown: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    for piece in read(markdown) {
        match piece {
            Piece::Paragraph { spans: more, .. }
            | Piece::Item { spans: more, .. }
            | Piece::Heading { spans: more, .. }
            | Piece::Note { spans: more, .. } => {
                if !spans.is_empty() {
                    spans.push(Span::text(" "));
                }
                spans.extend(more);
            }
            Piece::Formula(latex) => spans.push(Span::plain(SpanKind::Math(latex))),
            Piece::Code { code, .. } => spans.push(Span::plain(SpanKind::Code(code))),
            Piece::Table(_) | Piece::Rule => {}
        }
    }
    spans
}
