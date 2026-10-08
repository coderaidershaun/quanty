//! Reads the few Markdown marks that stored text uses: emphasis, code, formulas, footnotes and
//! one-line list items. A full Markdown reader would also read `_` and `#`, which are text here.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Parsed {
    pub(super) lines: Vec<ParsedLine>,
    /// What a screen reader says: no marks, a formula as its LaTeX, no footnote marks.
    pub(super) plain: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedLine {
    pub(super) kind: LineKind,
    pub(super) spans: Vec<Span>,
    /// A blank line came before this line.
    pub(super) starts_paragraph: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LineKind {
    Text,
    /// A printed number or bullet, and the text that hangs after it.
    ListItem {
        marker: String,
    },
    /// A footnote: its mark, and the text of the note.
    Note {
        mark: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Span {
    Text {
        text: String,
        style: SpanStyle,
    },
    /// A formula, as the LaTeX between the delimiters.
    Math(String),
    /// A footnote mark in running text.
    Foot(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SpanStyle {
    Plain,
    Emphasis,
    Strong,
    StrongEmphasis,
    Code,
}

impl SpanStyle {
    /// The style of text between `stars` stars, when the text around them has this style: one
    /// star slants, two make strong, three do both.
    fn with_stars(self, stars: usize) -> SpanStyle {
        let italic = matches!(self, SpanStyle::Emphasis | SpanStyle::StrongEmphasis) || stars != 2;
        let bold = matches!(self, SpanStyle::Strong | SpanStyle::StrongEmphasis) || stars >= 2;
        match (italic, bold) {
            (false, false) => SpanStyle::Plain,
            (true, false) => SpanStyle::Emphasis,
            (false, true) => SpanStyle::Strong,
            (true, true) => SpanStyle::StrongEmphasis,
        }
    }
}

/// A run of more stars than this opens no emphasis.
const MAX_STARS: usize = 3;
/// Marks where a printed word may break. It is not part of the word, so it is taken out.
const SOFT_HYPHEN: char = '\u{ad}';

/// Anything that is not one of the marks stays as it is written.
pub(super) fn parse(markdown: &str) -> Parsed {
    let cleaned = clean(markdown);
    let mut lines = Vec::new();
    let mut after_blank_line = false;
    for line in cleaned.split('\n') {
        if line.trim().is_empty() {
            after_blank_line = true;
            continue;
        }
        lines.push(parse_line(line, after_blank_line && !lines.is_empty()));
        after_blank_line = false;
    }
    let plain = plain_text(&lines);
    Parsed { lines, plain }
}

/// Reads the text of one table cell: one line, whatever it starts with.
pub(super) fn parse_cell(text: &str) -> Parsed {
    let line = ParsedLine {
        kind: LineKind::Text,
        spans: parse_inline(
            &clean(text).trim().chars().collect::<Vec<_>>(),
            SpanStyle::Plain,
        ),
        starts_paragraph: false,
    };
    let lines = if line.spans.is_empty() {
        vec![]
    } else {
        vec![line]
    };
    let plain = plain_text(&lines);
    Parsed { lines, plain }
}

pub(super) fn clean(text: &str) -> String {
    text.replace("\r\n", "\n")
        .chars()
        .filter(|&c| c != SOFT_HYPHEN)
        .map(|c| if c == '\t' || c == '\r' { ' ' } else { c })
        .collect()
}

fn parse_line(line: &str, starts_paragraph: bool) -> ParsedLine {
    let (kind, rest) = line_kind(line.trim());
    let chars: Vec<char> = rest.chars().collect();
    ParsedLine {
        kind,
        spans: parse_inline(&chars, SpanStyle::Plain),
        starts_paragraph,
    }
}

fn line_kind(line: &str) -> (LineKind, &str) {
    if let Some(rest) = line.strip_prefix("[^")
        && let Some((mark, note)) = rest.split_once("]: ")
        && is_foot_mark(mark)
    {
        let kind = LineKind::Note {
            mark: mark.to_owned(),
        };
        return (kind, note.trim_start());
    }
    if let Some((marker, rest)) = line.split_once(' ')
        && is_list_marker(marker)
    {
        let kind = LineKind::ListItem {
            marker: marker.to_owned(),
        };
        return (kind, rest.trim_start());
    }
    (LineKind::Text, line)
}

fn is_list_marker(word: &str) -> bool {
    matches!(word, "-" | "•" | "*")
        || word
            .strip_suffix('.')
            .is_some_and(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
}

fn is_foot_mark(mark: &str) -> bool {
    !mark.is_empty() && !mark.contains(|c: char| c.is_whitespace() || c == '[' || c == ']')
}

/// `base` is the style that the text already has, when it stands inside an emphasis.
fn parse_inline(chars: &[char], base: SpanStyle) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut text = String::new();
    let mut at = 0;
    while at < chars.len() {
        let rest = &chars[at..];
        match chars[at] {
            '\\' => match rest.get(1) {
                Some(&c @ ('*' | '`' | '[')) => {
                    text.push(c);
                    at += 2;
                }
                _ => match formula(rest) {
                    Some((latex, length)) => {
                        flush(&mut spans, &mut text, base);
                        spans.push(Span::Math(latex));
                        at += length;
                    }
                    None => {
                        text.push('\\');
                        at += 1;
                    }
                },
            },
            '`' => match code_end(chars, at + 1) {
                Some(end) => {
                    flush(&mut spans, &mut text, base);
                    let code: String = chars[at + 1..end].iter().collect();
                    push_text(&mut spans, code, SpanStyle::Code);
                    at = end + 1;
                }
                None => {
                    text.push('`');
                    at += 1;
                }
            },
            '[' => match foot_mark(rest) {
                Some((mark, length)) => {
                    flush(&mut spans, &mut text, base);
                    spans.push(Span::Foot(mark));
                    at += length;
                }
                None => {
                    text.push('[');
                    at += 1;
                }
            },
            '*' => at += emphasis(chars, at, base, &mut spans, &mut text),
            other => {
                text.push(other);
                at += 1;
            }
        }
    }
    flush(&mut spans, &mut text, base);
    spans
}

fn flush(spans: &mut Vec<Span>, text: &mut String, style: SpanStyle) {
    if !text.is_empty() {
        push_text(spans, std::mem::take(text), style);
    }
}

fn push_text(spans: &mut Vec<Span>, text: String, style: SpanStyle) {
    if let Some(Span::Text {
        text: last,
        style: before,
    }) = spans.last_mut()
        && *before == style
    {
        last.push_str(&text);
        return;
    }
    spans.push(Span::Text { text, style });
}

/// A formula at the start of `rest`: its LaTeX, and how many characters it takes. Delimiters
/// with nothing between them are not a formula.
fn formula(rest: &[char]) -> Option<(String, usize)> {
    if rest.get(1) != Some(&'(') {
        return None;
    }
    let end = formula_end(rest, 2)?;
    let latex = rest[2..end].iter().collect::<String>().trim().to_owned();
    (!latex.is_empty()).then_some((latex, end + 2))
}

fn formula_end(chars: &[char], from: usize) -> Option<usize> {
    (from..chars.len().saturating_sub(1)).find(|&i| chars[i] == '\\' && chars[i + 1] == ')')
}

/// A code span holds at least one character.
fn code_end(chars: &[char], from: usize) -> Option<usize> {
    (from + 1..chars.len()).find(|&i| chars[i] == '`')
}

/// A footnote mark at the start of `rest`: its text, and how many characters it takes.
fn foot_mark(rest: &[char]) -> Option<(String, usize)> {
    if rest.get(1) != Some(&'^') {
        return None;
    }
    let close = rest.iter().position(|&c| c == ']')?;
    let mark: String = rest[2..close].iter().collect();
    is_foot_mark(&mark).then_some((mark, close + 1))
}

/// Reads the emphasis that may start at `chars[at]`, and returns how many characters it used.
fn emphasis(
    chars: &[char],
    at: usize,
    base: SpanStyle,
    spans: &mut Vec<Span>,
    text: &mut String,
) -> usize {
    let stars = chars[at..].iter().take_while(|&&c| c == '*').count();
    let opens = chars.get(at + stars).is_some_and(|c| !c.is_whitespace());
    let close = if opens && stars <= MAX_STARS {
        closing_stars(chars, at + stars, stars)
    } else {
        None
    };
    let Some(close) = close else {
        text.extend(std::iter::repeat_n('*', stars));
        return stars;
    };
    flush(spans, text, base);
    spans.extend(parse_inline(
        &chars[at + stars..close],
        base.with_stars(stars),
    ));
    close + stars - at
}

/// A formula or a code span in between hides the stars inside it.
fn closing_stars(chars: &[char], from: usize, stars: usize) -> Option<usize> {
    let mut at = from;
    while at < chars.len() {
        match chars[at] {
            '\\' if chars.get(at + 1) == Some(&'(') => {
                at = formula_end(chars, at + 2).map_or(at + 2, |end| end + 2);
            }
            '\\' => at += 2,
            '`' => at = code_end(chars, at + 1).map_or(at + 1, |end| end + 1),
            '*' => {
                let run = chars[at..].iter().take_while(|&&c| c == '*').count();
                if run == stars && at > from && !chars[at - 1].is_whitespace() {
                    return Some(at);
                }
                at += run;
            }
            _ => at += 1,
        }
    }
    None
}

fn plain_text(lines: &[ParsedLine]) -> String {
    let mut plain = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            plain.push_str(if line.starts_paragraph { "\n\n" } else { "\n" });
        }
        let mut text = String::new();
        for span in &line.spans {
            match span {
                Span::Text { text: words, .. } => text.push_str(words),
                Span::Math(latex) => text.push_str(latex),
                Span::Foot(_) => {}
            }
        }
        if let LineKind::ListItem { marker } = &line.kind {
            plain.push_str(marker);
            plain.push(' ');
        }
        plain.push_str(text.trim());
    }
    plain
}
