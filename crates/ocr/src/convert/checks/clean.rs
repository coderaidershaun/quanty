//! Tidies every model reply before it is checked or saved, without changing any word or symbol a
//! reader can see.

use super::latex;
use crate::convert::reply::{CitedLabel, TranscribedPage, TranscribedPiece};

/// Removes invisible characters. A soft hyphen marks a word broken at a line end, and copying it
/// makes a word no search can find; the whitespace after it goes too so the halves rejoin.
pub(super) fn remove_invisible(text: &str) -> String {
    let mut visible = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\u{AD}' => while characters.next_if(|next| next.is_whitespace()).is_some() {},
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' => {}
            _ => visible.push(character),
        }
    }
    visible
}

pub(super) fn clean_reply(page: &mut TranscribedPage) {
    clean_optional(&mut page.printed_page_number);
    clean_optional(&mut page.running_header);
    for piece in &mut page.pieces {
        match piece {
            TranscribedPiece::Heading {
                printed_number,
                text,
                ..
            } => {
                clean_optional(printed_number);
                clean_text(text);
            }
            TranscribedPiece::Text {
                markdown, cites, ..
            } => {
                clean_text(markdown);
                clean_cites(cites);
            }
            TranscribedPiece::Formula {
                latex,
                label,
                name,
                statement,
                symbols,
                ..
            } => {
                *latex = latex::mend_row_breaks(&remove_invisible(latex));
                // A row break with no aligned block around it has nothing to break.
                if latex::has_bare_row_break(latex) {
                    *latex = format!("\\begin{{aligned}} {latex} \\end{{aligned}}");
                }
                clean_text(latex);
                clean_optional(label);
                clean_optional(name);
                clean_text(statement);
                for symbol in symbols {
                    clean_text(&mut symbol.symbol);
                    clean_text(&mut symbol.meaning);
                }
            }
            TranscribedPiece::Figure {
                label,
                caption,
                printed_text,
                explanation,
                ..
            } => {
                clean_optional(label);
                clean_optional(caption);
                printed_text.iter_mut().for_each(clean_text);
                clean_text(explanation);
            }
            TranscribedPiece::Table {
                label,
                caption,
                markdown,
                note,
                summary,
                ..
            } => {
                clean_optional(label);
                clean_optional(caption);
                clean_table_lines(markdown);
                clean_optional(note);
                clean_text(summary);
            }
            TranscribedPiece::Footnote {
                marker,
                markdown,
                cites,
                ..
            } => {
                clean_optional(marker);
                clean_text(markdown);
                clean_cites(cites);
            }
        }
    }
}

fn clean_text(text: &mut String) {
    *text = remove_invisible(text).trim().to_owned();
}

fn clean_optional(text: &mut Option<String>) {
    if let Some(inner) = text {
        clean_text(inner);
        if inner.is_empty() {
            *text = None;
        }
    }
}

fn clean_cites(cites: &mut [CitedLabel]) {
    for cite in cites {
        clean_text(&mut cite.label);
    }
}

fn clean_table_lines(markdown: &mut String) {
    let lines: Vec<String> = remove_invisible(markdown)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    *markdown = lines.join("\n");
}
