//! The formulas, figures, tables and code listings of a converted chapter, one item each, with the
//! parts of a listing that page breaks cut joined again.

use crate::content::PieceDetail;
use crate::reader::{Chapter, ChapterPiece};

/// A formula, a figure, a table or a code listing, each one item on its own. Text, headings and
/// footnotes are not items.
#[derive(Debug, Clone, PartialEq)]
pub struct ChapterItem<'a> {
    /// The piece it starts with: its page, number, section, kind, label and caption.
    pub first: &'a ChapterPiece,
    /// The later parts of a code listing that page breaks cut, in order. Empty for every other
    /// item.
    pub rest: Vec<&'a ChapterPiece>,
}

impl<'a> ChapterItem<'a> {
    /// The first piece, then the later parts of a cut listing.
    pub fn parts(&self) -> impl Iterator<Item = &'a ChapterPiece> + '_ {
        std::iter::once(self.first).chain(self.rest.iter().copied())
    }

    /// What the item shows: the content of the first piece, or the content of every part of a cut
    /// listing joined by a new line.
    pub fn content(&self) -> String {
        self.parts()
            .map(|piece| piece.content.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The page a cut listing ends on. `None` for an item on one page.
    pub fn last_page(&self) -> Option<u32> {
        self.rest.last().map(|piece| piece.id.page)
    }

    /// The page number printed on the page a cut listing ends on, when it shows one.
    pub fn last_printed_page(&self) -> Option<&'a str> {
        self.rest
            .last()
            .copied()
            .and_then(|piece| piece.printed_page_number.as_deref())
    }

    /// The first language that any part of a code listing names. `None` for any other item.
    pub fn language(&self) -> Option<&'a str> {
        self.parts().find_map(code_language)
    }

    /// True when `next` can be the rest of this listing: it is on the page after the last part, it
    /// has no label and no caption of its own, and its language is the listing's, or one of the two
    /// is not known.
    fn is_continued_by(&self, next: &ChapterPiece) -> bool {
        let last = self.rest.last().copied().unwrap_or(self.first);
        let is_on_next_page = next.id.page == last.id.page + 1;
        let do_languages_agree = match (self.language(), code_language(next)) {
            (Some(known), Some(other)) => known == other,
            _ => true,
        };
        is_on_next_page && !is_named(next) && do_languages_agree
    }
}

impl Chapter {
    /// Fingerprints source content and inventory without paths or generated sidecars.
    pub fn artifact_source(&self) -> crate::content::ArtifactSource {
        use crate::content::{ArtifactPiece, ArtifactSource, ArtifactTarget};
        use sha2::{Digest, Sha256};
        let manifest: Vec<_> = self
            .items()
            .iter()
            .map(|item| {
                let kind = match item.first.detail {
                    PieceDetail::Formula { .. } => "formula",
                    PieceDetail::Figure { .. } => "figure",
                    PieceDetail::Table { .. } => "table",
                    _ => "code",
                };
                ArtifactTarget {
                    id: format!(
                        "{}:{}:{}",
                        self.index.source_sha256, item.first.id.page, item.first.id.number
                    ),
                    kind: kind.to_owned(),
                    pieces: item
                        .parts()
                        .map(|piece| ArtifactPiece {
                            page: piece.id.page,
                            number: piece.id.number,
                        })
                        .collect(),
                }
            })
            .collect();
        let mut hash = Sha256::new();
        let pieces: Vec<_> = self.pieces.iter().map(|piece| {
            let mut detail = serde_json::to_value(&piece.detail).expect("piece details always serialize");
            normalize_detail(&mut detail);
            serde_json::json!({"page":piece.id.page,"number":piece.id.number,"printed":piece.printed_page_number,"detail":detail,"content":piece.content})
        }).collect();
        let mut content = serde_json::json!({"media":self.index.media_title,"name":self.index.name,"pieces":pieces});
        content.sort_all_objects();
        hash.update(content.to_string().as_bytes());
        hash.update(serde_json::to_vec(&manifest).expect("source inventory always serializes"));
        ArtifactSource {
            signature: hash
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            manifest,
        }
    }

    /// Every formula, figure, table and code listing, in reading order. A code piece on the next
    /// page, with no label and no caption, whose language agrees or is not known, with only
    /// footnotes between, continues the listing before it.
    pub fn items(&self) -> Vec<ChapterItem<'_>> {
        let mut items: Vec<ChapterItem<'_>> = Vec::new();
        // Only a footnote may sit between the parts of a listing: any other piece ends it.
        let mut is_in_listing = false;
        for piece in &self.pieces {
            match &piece.detail {
                PieceDetail::Formula { .. }
                | PieceDetail::Figure { .. }
                | PieceDetail::Table { .. } => {
                    items.push(ChapterItem {
                        first: piece,
                        rest: Vec::new(),
                    });
                    is_in_listing = false;
                }
                PieceDetail::Code { .. } => {
                    let cut_listing = items
                        .last_mut()
                        .filter(|listing| is_in_listing && listing.is_continued_by(piece));
                    match cut_listing {
                        Some(listing) => listing.rest.push(piece),
                        None => items.push(ChapterItem {
                            first: piece,
                            rest: Vec::new(),
                        }),
                    }
                    is_in_listing = true;
                }
                PieceDetail::Footnote { .. } => {}
                PieceDetail::Heading { .. } | PieceDetail::Text { .. } => is_in_listing = false,
            }
        }
        items
    }
}

fn normalize_detail(value: &mut serde_json::Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("image");
        object.retain(|_, value| {
            !value.is_null()
                && value != ""
                && value.as_array().is_none_or(|array| !array.is_empty())
        });
        for child in object.values_mut() {
            normalize_detail(child);
        }
    }
}

/// The language exactly as it is saved: blank text is a language too, and it does not agree with
/// any other.
fn code_language(piece: &ChapterPiece) -> Option<&str> {
    match &piece.detail {
        PieceDetail::Code { language, .. } => language.as_deref(),
        _ => None,
    }
}

/// True for a code piece with a label or a caption that is not blank.
fn is_named(piece: &ChapterPiece) -> bool {
    let PieceDetail::Code { label, caption, .. } = &piece.detail else {
        return false;
    };
    [label, caption]
        .into_iter()
        .flatten()
        .any(|text| !text.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::content::{ChapterIndex, DocumentName, FORMAT_VERSION};
    use crate::reader::PieceId;

    fn piece(
        page: u32,
        number: u32,
        detail: PieceDetail,
        content: &str,
        printed: &str,
    ) -> ChapterPiece {
        ChapterPiece {
            id: PieceId { page, number },
            printed_page_number: Some(printed.to_owned()),
            section: Vec::new(),
            content: content.to_owned(),
            file: PathBuf::new(),
            page_image: PathBuf::new(),
            figure_image: None,
            detail,
            starts_mid_sentence: false,
            ends_mid_sentence: false,
            relationships: Vec::new(),
        }
    }

    fn code(label: Option<&str>, language: Option<&str>) -> PieceDetail {
        PieceDetail::Code {
            label: label.map(str::to_owned),
            caption: None,
            language: language.map(str::to_owned),
            summary: String::new(),
        }
    }

    fn chapter(pieces: Vec<ChapterPiece>) -> Chapter {
        Chapter {
            index: ChapterIndex {
                format_version: FORMAT_VERSION,
                media_title: "Sample".to_owned(),
                name: DocumentName::Title("Sample".to_owned()),
                source_file: "sample.pdf".to_owned(),
                source_sha256: "0".repeat(64),
                page_count: 7,
                finished: true,
            },
            pieces,
        }
    }

    #[test]
    fn a_chapter_lists_its_formulas_figures_tables_and_listings_in_reading_order_and_a_cut_listing_once()
     {
        let formula = PieceDetail::Formula {
            label: Some("(1.1)".to_owned()),
            name: None,
            statement: String::new(),
            symbols: Vec::new(),
        };
        let figure = PieceDetail::Figure {
            label: None,
            caption: None,
            printed_text: Vec::new(),
            bounds: None,
            image: None,
        };
        let table = PieceDetail::Table {
            label: None,
            caption: None,
            summary: String::new(),
        };
        let heading = PieceDetail::Heading {
            rank: 1,
            printed_number: None,
        };
        let text = PieceDetail::Text { cites: Vec::new() };
        let footnote = PieceDetail::Footnote {
            marker: None,
            cites: Vec::new(),
        };
        // A blank label names nothing, so the part on page 3 still continues the listing.
        let blank_label = PieceDetail::Code {
            label: Some("  ".to_owned()),
            caption: None,
            language: Some("python".to_owned()),
            summary: String::new(),
        };
        let chapter = chapter(vec![
            piece(1, 1, heading, "Title", "4"),
            piece(1, 2, text, "Words.", "4"),
            piece(1, 3, formula, "x^2", "4"),
            piece(2, 1, code(Some("Listing 1"), None), "a = 1", "5"),
            piece(2, 2, footnote, "A note.", "5"),
            piece(3, 1, blank_label, "b = 2", "6"),
            piece(3, 2, table, "| a |", "6"),
            piece(4, 1, code(None, None), "c = 3", "7"),
            piece(5, 1, code(Some("Listing 2"), Some("python")), "d = 4", "8"),
            piece(6, 1, code(None, Some("rust")), "e = 5", "9"),
            piece(7, 1, figure, "A plot.", "10"),
        ]);

        let items = chapter.items();

        let starts: Vec<(u32, u32)> = items
            .iter()
            .map(|item| (item.first.id.page, item.first.id.number))
            .collect();
        assert_eq!(
            starts,
            [(1, 3), (2, 1), (3, 2), (4, 1), (5, 1), (6, 1), (7, 1)],
            "text, headings and footnotes are not items; the part on page 3 is not an item of its own"
        );

        let listing = &items[1];
        assert_eq!(listing.rest.len(), 1);
        assert_eq!(listing.content(), "a = 1\nb = 2");
        assert_eq!(listing.last_page(), Some(3));
        assert_eq!(listing.last_printed_page(), Some("6"));
        assert_eq!(
            listing.language(),
            Some("python"),
            "the first part names none, so the later part's language is the listing's"
        );

        let alone = items.iter().filter(|item| item.rest.is_empty()).count();
        assert_eq!(alone, 6, "a listing that is not continued stays alone");
        assert_eq!(items[0].content(), "x^2");
        assert_eq!(items[0].last_page(), None);
        assert_eq!(items[0].language(), None);
    }
}
