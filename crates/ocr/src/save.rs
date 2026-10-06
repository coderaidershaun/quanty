//! Turns one cleaned and checked reply into the files of a page: one file per piece and the
//! `page.json` that indexes them, with the relationships the page's own structure gives.

use std::path::Path;

use crate::content::{
    Cite, CiteKind, ContentError, Conversion, FORMAT_VERSION, FigureImage, PageIndex, PieceDetail,
    PieceEntry, Relationship, RelationshipKind,
};
use crate::transcribe::{CitedKind, CitedLabel, TranscribedPage, TranscribedPiece};

/// Writes every piece file and `page.json` into `folder`. `images` holds the picture of each
/// figure, by piece number.
pub(crate) fn write_page(
    folder: &Path,
    position: u32,
    page: &TranscribedPage,
    images: &[(u32, FigureImage)],
    conversion: Conversion,
) -> Result<(), ContentError> {
    let mut pieces = Vec::new();
    for piece in &page.pieces {
        let image = images
            .iter()
            .find(|(number, _)| *number == piece.number())
            .map(|(_, image)| image.clone());
        let (detail, content) = piece_parts(piece, image);
        let entry = PieceEntry::new(piece.number(), detail);
        let path = folder.join(&entry.file);
        std::fs::write(&path, format!("{}\n", content.trim()))
            .map_err(|source| ContentError::Write { path, source })?;
        pieces.push(entry);
    }
    let index = PageIndex {
        format_version: FORMAT_VERSION,
        page_position: position,
        printed_page_number: page.printed_page_number.clone(),
        running_header: page.running_header.clone(),
        starts_mid_sentence: page.starts_mid_sentence,
        ends_mid_sentence: page.ends_mid_sentence,
        pieces,
        relationships: relationships(page),
        conversion: Some(conversion),
    };
    index.write(folder)
}

/// The saved fields of a piece and what goes in its file. `image` is the picture of a figure.
fn piece_parts(piece: &TranscribedPiece, image: Option<FigureImage>) -> (PieceDetail, String) {
    match piece {
        TranscribedPiece::Heading {
            rank,
            printed_number,
            text,
            ..
        } => (
            PieceDetail::Heading {
                rank: *rank,
                printed_number: printed_number.clone(),
            },
            text.clone(),
        ),
        TranscribedPiece::Text {
            markdown, cites, ..
        } => (
            PieceDetail::Text {
                cites: all_cites(markdown, cites),
            },
            markdown.clone(),
        ),
        TranscribedPiece::Formula {
            latex,
            label,
            name,
            statement,
            symbols,
            ..
        } => (
            PieceDetail::Formula {
                label: label.clone(),
                name: name.clone(),
                statement: statement.clone(),
                symbols: symbols.clone(),
            },
            latex.clone(),
        ),
        TranscribedPiece::Figure {
            label,
            caption,
            printed_text,
            bounds,
            explanation,
            ..
        } => (
            PieceDetail::Figure {
                label: label.clone(),
                caption: caption.clone(),
                printed_text: printed_text.clone(),
                bounds: Some(*bounds),
                image,
            },
            figure_file(explanation, printed_text),
        ),
        TranscribedPiece::Table {
            label,
            caption,
            markdown,
            note,
            summary,
            ..
        } => (
            PieceDetail::Table {
                label: label.clone(),
                caption: caption.clone(),
                summary: summary.clone(),
            },
            match note {
                Some(note) => format!("{markdown}\n\n{note}"),
                None => markdown.clone(),
            },
        ),
        TranscribedPiece::Footnote {
            marker,
            markdown,
            cites,
            ..
        } => (
            PieceDetail::Footnote {
                marker: marker.clone(),
                cites: all_cites(markdown, cites),
            },
            markdown.clone(),
        ),
    }
}

/// The explanation, then one closing line that lists every printed word, so that none is missing
/// from the text that gets searched even when the explanation paraphrased it.
fn figure_file(explanation: &str, printed_text: &[String]) -> String {
    if printed_text.is_empty() {
        return explanation.to_owned();
    }
    let quoted: Vec<String> = printed_text
        .iter()
        .map(|text| format!("\"{text}\""))
        .collect();
    format!(
        "{explanation}\n\nText printed in the figure: {}",
        quoted.join("; ")
    )
}

/// The citations the model gave, then one footnote citation for each marker in the text.
fn all_cites(markdown: &str, model_cites: &[CitedLabel]) -> Vec<Cite> {
    let mut cites: Vec<Cite> = Vec::new();
    let from_model = model_cites.iter().map(|cite| Cite {
        kind: match cite.kind {
            CitedKind::Figure => CiteKind::Figure,
            CitedKind::Table => CiteKind::Table,
            CitedKind::Equation => CiteKind::Equation,
        },
        label: cite.label.clone(),
    });
    let from_markers = footnote_markers(markdown).into_iter().map(|marker| Cite {
        kind: CiteKind::Footnote,
        label: marker,
    });
    for cite in from_model.chain(from_markers) {
        if !cites.contains(&cite) {
            cites.push(cite);
        }
    }
    cites
}

/// The distinct footnote markers in `text`, written `[^1]`, in the order they first appear.
fn footnote_markers(text: &str) -> Vec<String> {
    let mut markers: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[^") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(']') else { break };
        let marker = &rest[..end];
        if !marker.is_empty() && !markers.iter().any(|known| known == marker) {
            markers.push(marker.to_owned());
        }
        rest = &rest[end..];
    }
    markers
}

/// Every link between the pieces of one page, sorted and without repeats.
fn relationships(page: &TranscribedPage) -> Vec<Relationship> {
    let mut links = lead_ins(page);
    links.extend(page.discusses.iter().map(|discussion| Relationship {
        kind: RelationshipKind::Discusses,
        from: discussion.piece,
        to: discussion.about,
    }));
    links.extend(footnote_links(page));
    links.sort_by_key(|link| (link.from, link.to, link.kind));
    links.dedup();
    links
}

/// One link for each formula whose lead-in is on this page: the text piece just before it.
fn lead_ins(page: &TranscribedPage) -> Vec<Relationship> {
    let mut links = Vec::new();
    for (index, piece) in page.pieces.iter().enumerate() {
        if !matches!(piece, TranscribedPiece::Formula { .. }) {
            continue;
        }
        // Formulas printed one under another share the text before the first of them.
        let lead_in = page.pieces[..index]
            .iter()
            .rev()
            .find(|earlier| !matches!(earlier, TranscribedPiece::Formula { .. }));
        if let Some(text @ TranscribedPiece::Text { .. }) = lead_in {
            links.push(Relationship {
                kind: RelationshipKind::Introduces,
                from: text.number(),
                to: piece.number(),
            });
        }
    }
    links
}

/// One link from each footnote to every heading, text or table piece that carries its marker.
fn footnote_links(page: &TranscribedPage) -> Vec<Relationship> {
    let mut links = Vec::new();
    for footnote in &page.pieces {
        let TranscribedPiece::Footnote {
            marker: Some(marker),
            ..
        } = footnote
        else {
            continue;
        };
        let carrying = format!("[^{marker}]");
        for carrier in &page.pieces {
            let words = match carrier {
                TranscribedPiece::Heading { text, .. } => text,
                TranscribedPiece::Text { markdown, .. }
                | TranscribedPiece::Table { markdown, .. } => markdown,
                TranscribedPiece::Formula { .. }
                | TranscribedPiece::Figure { .. }
                | TranscribedPiece::Footnote { .. } => continue,
            };
            if words.contains(&carrying) {
                links.push(Relationship {
                    kind: RelationshipKind::FootnoteOf,
                    from: footnote.number(),
                    to: carrier.number(),
                });
            }
        }
    }
    links
}
