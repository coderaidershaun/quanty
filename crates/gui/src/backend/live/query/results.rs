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

/// A chunk has none, because one chunk can hold several text pieces. A formula, figure or table
/// has one only when exactly one piece of its page has its kind and its label: with none or with
/// two there is no guess.
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

/// `facts` has one entry for each hit, and a hit past its end has none.
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
                media: payload.document_labels.media.clone(),
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
