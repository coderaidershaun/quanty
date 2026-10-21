//! The code listings, one item each, with the parts that page breaks cut joined again.

use ocr::{Chapter, ChapterPiece, PieceDetail};
use rag_core::{DocumentInput, ItemKind};

use super::pieces::{join_blocks, label_line, printed_label};
use super::{Draft, context_line};

/// One code piece, which is a whole listing or one page of it.
struct Part<'a> {
    piece: &'a ChapterPiece,
    label: Option<&'a str>,
    caption: Option<&'a str>,
    language: Option<&'a str>,
    summary: &'a str,
}

/// A listing as a reader sees it: one code piece, or several that page breaks cut apart.
struct Listing<'a> {
    first: Part<'a>,
    later: Vec<Part<'a>>,
}

impl<'a> Listing<'a> {
    fn parts(&self) -> impl Iterator<Item = &Part<'a>> {
        std::iter::once(&self.first).chain(&self.later)
    }

    fn last(&self) -> &Part<'a> {
        self.later.last().unwrap_or(&self.first)
    }

    /// The first language that any part names, which is the one the item stores.
    fn language(&self) -> Option<&'a str> {
        self.parts().find_map(|part| part.language)
    }

    /// True when `next` can be the rest of this listing: it is on the next page, it has no label
    /// and no caption of its own, and its language is the listing's, or one of the two is not
    /// known.
    fn is_continued_by(&self, next: &Part<'_>) -> bool {
        let on_next_page = next.piece.id.page == self.last().piece.id.page + 1;
        let unnamed = label_line(next.label, next.caption).is_none();
        let same_language = match (self.language(), next.language) {
            (Some(known), Some(other)) => known == other,
            _ => true,
        };
        on_next_page && unnamed && same_language
    }

    fn into_draft(self, doc_title: &str) -> Draft {
        let code = self
            .parts()
            .map(|part| part.piece.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let language = self.language();
        let heading = label_line(self.first.label, self.first.caption);
        let language_line = language.map(|name| format!("Language: {name}"));
        let blocks = [heading.as_deref(), language_line.as_deref()]
            .into_iter()
            .chain(self.parts().map(|part| Some(part.summary)))
            .chain([Some(code.as_str())]);
        let text = join_blocks(blocks);

        let last_later_piece = self.later.last().map(|part| part.piece);
        Draft {
            first_piece: self.first.piece.id,
            printed_page: self.first.piece.printed_page_number.clone(),
            last_page: last_later_piece.map(|piece| piece.id.page),
            last_printed_page: last_later_piece.and_then(|piece| piece.printed_page_number.clone()),
            kind: ItemKind::Code,
            text: code,
            label: printed_label(self.first.label),
            language: language.map(str::to_owned),
            cites: Vec::new(),
            input: DocumentInput {
                title: context_line(doc_title, &self.first.piece.section),
                text,
                image: None,
            },
        }
    }
}

pub(super) fn listing_drafts(chapter: &Chapter, doc_title: &str) -> Vec<Draft> {
    let mut listings: Vec<Listing> = Vec::new();
    // Only a footnote may sit between the parts of a listing: any other piece ends it.
    let mut is_in_listing = false;
    for piece in &chapter.pieces {
        match &piece.detail {
            PieceDetail::Code {
                label,
                caption,
                language,
                summary,
            } => {
                let part = Part {
                    piece,
                    label: label.as_deref(),
                    caption: caption.as_deref(),
                    language: language.as_deref(),
                    summary,
                };
                let cut_listing = listings
                    .last_mut()
                    .filter(|listing| is_in_listing && listing.is_continued_by(&part));
                match cut_listing {
                    Some(listing) => listing.later.push(part),
                    None => listings.push(Listing {
                        first: part,
                        later: Vec::new(),
                    }),
                }
                is_in_listing = true;
            }
            PieceDetail::Footnote { .. } => {}
            _ => is_in_listing = false,
        }
    }
    listings
        .into_iter()
        .map(|listing| listing.into_draft(doc_title))
        .collect()
}
