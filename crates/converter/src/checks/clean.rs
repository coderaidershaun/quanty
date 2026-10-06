//! Tidies every model reply before it is checked or saved, without changing any word or symbol a
//! reader can see.

use super::latex;
use crate::transcribe::{CitedLabel, TranscribedPage, TranscribedPiece};

/// Removes the characters nobody can see. The text layer marks a word broken at a line end with
/// a soft hyphen, and a model that copies it writes a word that looks right but that no search
/// can find. The whitespace after a soft hyphen goes with it, so the two halves rejoin.
pub(crate) fn remove_invisible(text: &str) -> String {
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

/// Cleans every string of a reply: invisible characters out, a line break after a row break in
/// a formula mended, a bare row break in a formula put inside an aligned block, table lines
/// tidied, every string trimmed, and an optional string that is empty turned into `None`.
pub(crate) fn clean_reply(page: &mut TranscribedPage) {
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
                // A formula printed over several lines belongs in an aligned block, and a row
                // break with no block around it has nothing to break.
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
