//! Lists every string of a reply with where it sits, and the rules that read those strings.

use super::fault::{ALIGNED_WITHOUT_ROW_BREAK, PieceRef, ReplyFault};
use super::latex;
use crate::convert::reply::{CitedLabel, TranscribedPage, TranscribedPiece};

#[derive(Clone, Copy, PartialEq)]
enum Role {
    /// LaTeX from end to end.
    Formula,
    /// LaTeX from end to end.
    Symbol,
    /// Prose that may hold `\( … \)` spans.
    Mixed,
    Plain,
}

struct Field<'a> {
    /// Index into the page's pieces; `None` for a page field.
    piece: Option<usize>,
    name: &'static str,
    text: &'a str,
    role: Role,
}

struct Collector<'a> {
    fields: Vec<Field<'a>>,
    piece: Option<usize>,
}

impl<'a> Collector<'a> {
    fn add(&mut self, name: &'static str, text: &'a str, role: Role) {
        self.fields.push(Field {
            piece: self.piece,
            name,
            text,
            role,
        });
    }

    fn add_optional(&mut self, name: &'static str, text: &'a Option<String>, role: Role) {
        if let Some(text) = text {
            self.add(name, text, role);
        }
    }

    fn add_cites(&mut self, cites: &'a [CitedLabel]) {
        for cite in cites {
            self.add("cites", &cite.label, Role::Plain);
        }
    }
}

fn fields(page: &TranscribedPage) -> Vec<Field<'_>> {
    let mut collector = Collector {
        fields: Vec::new(),
        piece: None,
    };
    collector.add_optional(
        "printed-page-number",
        &page.printed_page_number,
        Role::Plain,
    );
    collector.add_optional("running-header", &page.running_header, Role::Plain);
    for (index, piece) in page.pieces.iter().enumerate() {
        collector.piece = Some(index);
        match piece {
            TranscribedPiece::Heading {
                printed_number,
                text,
                ..
            } => {
                collector.add_optional("printed-number", printed_number, Role::Plain);
                collector.add("text", text, Role::Mixed);
            }
            TranscribedPiece::Text {
                markdown, cites, ..
            } => {
                collector.add("markdown", markdown, Role::Mixed);
                collector.add_cites(cites);
            }
            TranscribedPiece::Formula {
                latex,
                label,
                name,
                statement,
                symbols,
                ..
            } => {
                collector.add("latex", latex, Role::Formula);
                collector.add_optional("label", label, Role::Plain);
                collector.add_optional("name", name, Role::Plain);
                collector.add("statement", statement, Role::Plain);
                for symbol in symbols {
                    collector.add("symbols", &symbol.symbol, Role::Symbol);
                    collector.add("symbols", &symbol.meaning, Role::Plain);
                }
            }
            TranscribedPiece::Figure {
                label,
                caption,
                printed_text,
                explanation,
                ..
            } => {
                collector.add_optional("label", label, Role::Plain);
                collector.add_optional("caption", caption, Role::Mixed);
                for text in printed_text {
                    collector.add("printed-text", text, Role::Plain);
                }
                collector.add("explanation", explanation, Role::Mixed);
            }
            TranscribedPiece::Table {
                label,
                caption,
                markdown,
                note,
                summary,
                ..
            } => {
                collector.add_optional("label", label, Role::Plain);
                collector.add_optional("caption", caption, Role::Mixed);
                collector.add("markdown", markdown, Role::Mixed);
                collector.add_optional("note", note, Role::Mixed);
                collector.add("summary", summary, Role::Plain);
            }
            TranscribedPiece::Footnote {
                marker,
                markdown,
                cites,
                ..
            } => {
                collector.add_optional("marker", marker, Role::Plain);
                collector.add("markdown", markdown, Role::Mixed);
                collector.add_cites(cites);
            }
        }
    }
    collector.fields
}

fn empty_content(page: &TranscribedPage, fields: &[Field<'_>]) -> Result<(), ReplyFault> {
    for field in fields.iter().filter(|field| field.text.trim().is_empty()) {
        if let Some(piece) = piece_ref_of(page, field) {
            return Err(ReplyFault::EmptyContent {
                piece,
                field: field.name,
            });
        }
    }
    Ok(())
}

fn control_character_name(text: &str) -> Option<&'static str> {
    text.chars().find_map(|character| match character {
        '\t' => Some("a tab"),
        '\u{8}' => Some("a backspace"),
        '\u{c}' => Some("a form feed"),
        '\r' => Some("a carriage return"),
        _ => None,
    })
}

fn lost_backslash(page: &TranscribedPage, fields: &[Field<'_>]) -> Result<(), ReplyFault> {
    for field in fields {
        let piece = piece_ref_of(page, field);
        let fault = |seen| ReplyFault::LostBackslash {
            piece: piece.clone(),
            field: field.name,
            seen,
        };
        if let Some(seen) = control_character_name(field.text) {
            return Err(fault(seen));
        }
        match field.role {
            Role::Formula | Role::Symbol if field.text.contains('\n') => {
                return Err(fault("a line break inside LaTeX"));
            }
            Role::Formula if latex::aligned_without_row_break(field.text) => {
                return Err(fault(ALIGNED_WITHOUT_ROW_BREAK));
            }
            Role::Mixed => {
                // A span that does not close is reported by the balance rule.
                let spans = latex::math_spans(field.text).unwrap_or_default();
                if spans.iter().any(|span| span.contains('\n')) {
                    return Err(fault("a line break inside LaTeX"));
                }
            }
            Role::Formula | Role::Symbol | Role::Plain => {}
        }
    }
    Ok(())
}

fn latex_parts<'a>(field: &Field<'a>) -> Result<Vec<&'a str>, &'static str> {
    match field.role {
        Role::Formula | Role::Symbol => Ok(vec![field.text]),
        Role::Mixed => latex::math_spans(field.text),
        Role::Plain => Ok(Vec::new()),
    }
}

fn balance(page: &TranscribedPage, fields: &[Field<'_>]) -> Result<(), ReplyFault> {
    for field in fields {
        let Some(piece) = piece_ref_of(page, field) else {
            continue;
        };
        let fault = |seen| ReplyFault::Unbalanced {
            piece: piece.clone(),
            field: field.name,
            seen,
        };
        for part in latex_parts(field).map_err(fault)? {
            if let Some(seen) = latex::unbalanced(part) {
                return Err(fault(seen));
            }
        }
    }
    Ok(())
}

fn math_will_not_typeset(page: &TranscribedPage, fields: &[Field<'_>]) -> Result<(), ReplyFault> {
    for field in fields {
        let Some(piece) = piece_ref_of(page, field) else {
            continue;
        };
        let fault = |seen| ReplyFault::MathWillNotTypeset {
            piece: piece.clone(),
            field: field.name,
            seen,
        };
        if field.role == Role::Formula
            && let Some(seen) = latex::wrapped_or_labelled(field.text)
        {
            return Err(fault(seen));
        }
        // An unclosed span is reported by the balance rule, so it is skipped here.
        for part in latex_parts(field).unwrap_or_default() {
            if let Some(seen) = latex::bare_percent_or_dollar(part) {
                return Err(fault(seen));
            }
        }
    }
    Ok(())
}

fn piece_ref_of(page: &TranscribedPage, field: &Field<'_>) -> Option<PieceRef> {
    field.piece.map(|index| PieceRef::of(page, index))
}

pub(crate) fn any_string_has_backslash(page: &TranscribedPage) -> bool {
    fields(page).iter().any(|field| field.text.contains('\\'))
}

pub(super) fn check_strings(page: &TranscribedPage) -> Result<(), ReplyFault> {
    let fields = fields(page);
    empty_content(page, &fields)?;
    lost_backslash(page, &fields)?;
    balance(page, &fields)?;
    math_will_not_typeset(page, &fields)
}
