//! Turns the hits of a search into the results the window shows. What the stored hit does not
//! hold, such as the number of its piece on the page, is read from the chapter on disk.

use std::collections::HashMap;
use std::path::PathBuf;

use ocr::{Chapter, ChapterPiece, PieceDetail};
use rag_core::{DocId, ItemKind, ItemPayload};
use rag_retrieval::SearchHit;

use crate::contract::{self, ImageRef, ResultItem};

/// What the chapter on disk says about the piece a hit was made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PieceFacts {
    pub(super) piece: u32,
    pub(super) caption: Option<String>,
    pub(super) name: Option<String>,
}

/// For each hit, the piece it was made from, or `None`. A chunk has none, because one chunk can
/// hold several text pieces. A formula, figure or table has one when exactly one piece of its
/// page has its kind and its label; with none or with two there is no guess. A document with no
/// folder, or whose chapter cannot be read, gives `None` for all its hits. Each chapter is read
/// once.
pub(super) fn piece_facts(
    hits: &[SearchHit],
    folders: &HashMap<DocId, PathBuf>,
) -> Vec<Option<PieceFacts>> {
    let mut chapters: HashMap<DocId, Option<Chapter>> = HashMap::new();
    hits.iter()
        .map(|hit| {
            let payload = &hit.item.payload;
            if payload.kind == ItemKind::Chunk {
                return None;
            }
            let folder = folders.get(&payload.doc_id)?;
            let chapter = chapters
                .entry(payload.doc_id)
                .or_insert_with(|| {
                    ocr::read_chapter(folder)
                        .inspect_err(|error| {
                            tracing::warn!(?error, "could not read a chapter for its pieces");
                        })
                        .ok()
                })
                .as_ref()?;
            only_piece_of(chapter, payload)
        })
        .collect()
}

fn only_piece_of(chapter: &Chapter, payload: &ItemPayload) -> Option<PieceFacts> {
    let mut fitting = chapter
        .pieces
        .iter()
        .filter(|piece| piece.id.page == payload.page)
        .filter_map(|piece| facts_if_it_fits(piece, payload));
    let only = fitting.next()?;
    fitting.next().is_none().then_some(only)
}

/// The labels are compared as ingest stores them: with no space at either end, and a blank one is
/// no label.
fn facts_if_it_fits(piece: &ChapterPiece, payload: &ItemPayload) -> Option<PieceFacts> {
    let (kind, label, name, caption) = match &piece.detail {
        PieceDetail::Formula { label, name, .. } => {
            (ItemKind::Formula, label, name.as_deref(), None)
        }
        PieceDetail::Figure { label, caption, .. } => {
            (ItemKind::Figure, label, None, caption.as_deref())
        }
        PieceDetail::Table { label, caption, .. } => {
            (ItemKind::Table, label, None, caption.as_deref())
        }
        _ => return None,
    };
    let fits =
        kind == payload.kind && printed(label.as_deref()) == printed(payload.label.as_deref());
    fits.then(|| PieceFacts {
        piece: piece.id.number,
        caption: printed(caption).map(str::to_owned),
        name: printed(name).map(str::to_owned),
    })
}

fn printed(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

/// The results of a search, numbered from 1 in the order of the hits. `facts` has one entry for
/// each hit, and a hit past its end has none.
pub(super) fn result_items(hits: &[SearchHit], facts: Vec<Option<PieceFacts>>) -> Vec<ResultItem> {
    let facts = facts.into_iter().chain(std::iter::repeat(None));
    hits.iter()
        .zip(facts)
        .enumerate()
        .map(|(index, (hit, facts))| {
            let payload = &hit.item.payload;
            let (piece, caption, name) = match facts {
                Some(facts) => (Some(facts.piece), facts.caption, facts.name),
                None => (None, None, None),
            };
            ResultItem {
                number: index + 1,
                id: hit.item.id.into(),
                kind: payload.kind.into(),
                score: hit.item.score,
                reason: reason(&hit.reason),
                doc: payload.doc_id.into(),
                doc_title: payload.doc_title.clone(),
                book: payload.document_labels.book.clone(),
                page: payload.page,
                printed_page: payload.printed_page.clone(),
                label: payload.label.clone(),
                text: payload.text.clone(),
                image: payload.image_path.clone().map(|path| ImageRef { path }),
                piece,
                caption,
                name,
            }
        })
        .collect()
}

fn reason(reason: &rag_retrieval::Reason) -> contract::Reason {
    match reason {
        rag_retrieval::Reason::Nearest => contract::Reason::Nearest,
        rag_retrieval::Reason::Concept(name) => contract::Reason::Concept(name.clone()),
        rag_retrieval::Reason::Cited { by, label } => contract::Reason::Cited {
            by: *by,
            label: label.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rag_core::{DocumentLabels, ItemHit, ItemId};

    use super::*;

    fn sample_chapter(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/content")
            .join(path)
    }

    fn document(name: &str) -> DocId {
        DocId::from_source_sha256(name)
    }

    fn hit(document: DocId, page: u32, kind: ItemKind, label: Option<&str>) -> SearchHit {
        SearchHit {
            item: ItemHit {
                id: ItemId::new(document, kind, 0),
                score: 0.5,
                payload: ItemPayload {
                    doc_id: document,
                    doc_title: "A book".to_owned(),
                    page,
                    printed_page: None,
                    kind,
                    text: String::new(),
                    image_path: None,
                    label: label.map(str::to_owned),
                    cites: Vec::new(),
                    document_labels: DocumentLabels::default(),
                },
            },
            reason: rag_retrieval::Reason::Nearest,
        }
    }

    fn facts(piece: u32, caption: Option<&str>, name: Option<&str>) -> Option<PieceFacts> {
        Some(PieceFacts {
            piece,
            caption: caption.map(str::to_owned),
            name: name.map(str::to_owned),
        })
    }

    #[test]
    fn a_hit_takes_its_piece_caption_and_name_from_the_one_piece_of_its_page_kind_and_label() {
        let (notes_1, notes_2, volatility) = (
            document("notes 1"),
            document("notes 2"),
            document("volatility"),
        );
        let (no_folder, no_chapter) = (document("no folder"), document("no chapter"));
        let folders = HashMap::from([
            (notes_1, sample_chapter("quanty-sample-notes/chapter-1")),
            (notes_2, sample_chapter("quanty-sample-notes/chapter-2")),
            (
                volatility,
                sample_chapter("option-volatility-and-pricing/chapter-1"),
            ),
            (no_chapter, sample_chapter("quanty-sample-notes/chapter-9")),
        ]);
        let hits = [
            hit(notes_2, 3, ItemKind::Formula, Some("(2.4)")),
            hit(notes_1, 2, ItemKind::Table, Some("Table 1-1")),
            hit(notes_2, 3, ItemKind::Chunk, None),
            // Page 2 has two formulas and neither has a label.
            hit(volatility, 2, ItemKind::Formula, None),
            // Page 3 has two tables, told apart by their labels.
            hit(volatility, 3, ItemKind::Table, Some("Figure 7-2")),
            // Page 5 has a figure with this label and no formula.
            hit(volatility, 5, ItemKind::Formula, Some("Figure 13-4")),
            hit(volatility, 5, ItemKind::Figure, Some("Figure 13-4")),
            // Page 3 of the notes has no formula without a label.
            hit(notes_2, 3, ItemKind::Formula, None),
            hit(no_folder, 1, ItemKind::Formula, Some("(1.1)")),
            hit(no_chapter, 1, ItemKind::Formula, Some("(1.1)")),
        ];

        assert_eq!(
            piece_facts(&hits, &folders),
            vec![
                facts(5, None, Some("Black–Scholes call price")),
                facts(
                    3,
                    Some("How the price of a call and of a put respond when one input rises."),
                    None
                ),
                None,
                None,
                facts(
                    1,
                    Some("Effect of changing interest rates on option values."),
                    None
                ),
                None,
                facts(1, None, None),
                None,
                None,
                None,
            ]
        );
    }
}
