//! The contents of a sample chapter as the fake reads them: its items from the sample pages, and
//! the concepts of all its pages.

use std::cmp::Reverse;
use std::path::Path;

use super::scenes::{Concepts, Pages};
use super::{Fake, PAGE_WAIT, fixtures};
use crate::backend::Reply;
use crate::contract::{
    ChapterContents, ChapterItem, DocId, Event, PageConcept, PageView, PieceKind, RequestId,
};

impl Fake {
    pub(super) async fn load_chapter_contents(
        &self,
        request: RequestId,
        doc: DocId,
        reply: &Reply,
    ) {
        let script = &self.scene.script;
        self.wait(PAGE_WAIT).await;
        let items = match script.pages {
            Pages::SourceMissing => Err(self
                .source_missing("the scene says this document's files are not there".to_owned())),
            Pages::Samples => {
                contents(&self.samples, doc).map_err(|error| self.source_missing(error.to_string()))
            }
        };
        reply.send(Event::ChapterContents {
            request,
            result: items,
        });
        let concepts = match script.concepts {
            Concepts::Samples => {
                let page_count = self
                    .catalogue()
                    .document(doc)
                    .and_then(|document| document.pages)
                    .unwrap_or(0);
                Ok(document_concepts(doc, page_count))
            }
            Concepts::Empty => Ok(Vec::new()),
            Concepts::Fails(kind) => Err(self.failure(kind)),
        };
        reply.send(Event::ChapterConcepts {
            request,
            result: concepts,
        });
    }
}

// No sample listing runs over a page break, so none is joined here. A sample that has one would
// be listed differently by the fake and by the live reader.
fn contents(samples: &Path, doc: DocId) -> Result<ChapterContents, fixtures::SampleError> {
    let first = fixtures::page(samples, doc, 1)?;
    let page_count = first.page_count;
    let later = (2..=page_count)
        .map(|page| fixtures::page(samples, doc, page))
        .collect::<Result<Vec<_>, _>>()?;
    let items = std::iter::once(first)
        .chain(later)
        .flat_map(items_of)
        .collect();
    Ok(ChapterContents {
        doc,
        page_count,
        items,
    })
}

fn items_of(view: PageView) -> impl Iterator<Item = ChapterItem> {
    let PageView {
        page,
        printed_page,
        pieces,
        ..
    } = view;
    let is_item = |kind| {
        matches!(
            kind,
            PieceKind::Formula | PieceKind::Figure | PieceKind::Table | PieceKind::Code
        )
    };
    pieces
        .into_iter()
        .filter(move |piece| is_item(piece.kind))
        .map(move |piece| ChapterItem {
            page,
            printed_page: printed_page.clone(),
            last_page: None,
            last_printed_page: None,
            piece,
        })
}

/// Each concept once, the one that more pages list first, then by name in lower case, as the
/// graph orders by mentions and then by normalised name.
fn document_concepts(doc: DocId, page_count: u32) -> Vec<PageConcept> {
    let mut counted: Vec<(PageConcept, usize)> = Vec::new();
    for concept in (1..=page_count).flat_map(|page| fixtures::page_concepts(doc, page)) {
        match counted.iter_mut().find(|(known, _)| known.id == concept.id) {
            Some((_, pages)) => *pages += 1,
            None => counted.push((concept, 1)),
        }
    }
    counted.sort_by_cached_key(|(concept, pages)| (Reverse(*pages), concept.name.to_lowercase()));
    counted.into_iter().map(|(concept, _)| concept).collect()
}
