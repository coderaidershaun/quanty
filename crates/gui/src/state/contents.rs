//! The contents of the open document, as the Code, Formulas, Figures, Tables and Concepts views
//! read and export them: the items from the disk and the concepts from the graph, so one can be
//! ready while the other failed.

use export::{Block, Document, Format};

use super::shared::{Shared, push_cancel};
use contract::{
    Catalogue, ChapterContents, ChapterItem, Command, DocId, DocumentView, Effect, ExportPlace,
    ExportRequest, Failure, Loadable, PageConcept, PieceKind, RequestId,
};

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct Contents {
    /// The document the slot was loaded for.
    pub doc: Option<DocId>,
    /// `Some` while the items or the concepts are on the way.
    pub pending: Option<RequestId>,
    pub items: Loadable<ChapterContents>,
    pub concepts: Loadable<Vec<PageConcept>>,
}

impl Contents {
    /// True when the slot holds the contents of `doc`, loaded or on their way.
    pub fn is_for(&self, doc: Option<DocId>) -> bool {
        doc.is_some() && self.doc == doc
    }

    /// What a Code, Formulas, Figures, Tables or Concepts view asks of an export as `format`: what
    /// the view lists as a document, a file name made from the document's title, and the place
    /// that asked. `None` while the view's part is not ready, when it lists nothing, or when the
    /// catalogue does not list the document.
    pub fn export_request(
        &self,
        catalogue: &Catalogue,
        view: DocumentView,
        format: Format,
    ) -> Option<ExportRequest> {
        let doc = self.doc?;
        let title = &catalogue.document(doc)?.title;
        let blocks = match view.piece_kind() {
            Some(kind) => piece_blocks(self.items.ready()?, kind),
            None if view == DocumentView::Concepts => concept_blocks(self.concepts.ready()?),
            None => return None,
        };
        if blocks.is_empty() {
            return None;
        }
        let document = Document {
            title: title.clone(),
            origin: format!("{} from {title}", view.label()),
            blocks,
            sources: Vec::new(),
        };
        Some(ExportRequest {
            format,
            document,
            file_name: format.file_name(&format!("{title} {}", view.label().to_lowercase())),
            place: ExportPlace::Library { doc, view },
        })
    }
}

fn piece_blocks(chapter: &ChapterContents, kind: PieceKind) -> Vec<Block> {
    chapter.of_kind(kind).filter_map(piece_block).collect()
}

fn piece_block(item: &ChapterItem) -> Option<Block> {
    let piece = &item.piece;
    let caption = caption(item);
    match piece.kind {
        PieceKind::Formula => Some(Block::Formula {
            latex: piece.text.clone(),
            label: piece.label.clone(),
            caption,
            cite: None,
        }),
        PieceKind::Figure => Some(Block::Figure {
            picture: piece
                .image
                .as_ref()
                .map(|image| std::path::PathBuf::from(&image.url)),
            description: piece.text.clone(),
            caption,
            cite: None,
        }),
        PieceKind::Table => Some(Block::Table {
            markdown: piece.text.clone(),
            caption,
            cite: None,
        }),
        PieceKind::Code => Some(Block::Code {
            code: piece.text.clone(),
            language: piece.language.clone(),
            caption,
            cite: None,
        }),
        PieceKind::Heading { .. } | PieceKind::Text | PieceKind::Footnote => None,
    }
}

/// The words under a piece: its saved title in bold, or else its printed label, then its printed
/// caption, then the page it is printed on.
fn caption(item: &ChapterItem) -> String {
    let piece = &item.piece;
    let name = match (&item.metadata, &piece.label) {
        (Some(metadata), _) => Some(&metadata.title),
        (None, Some(label)) if piece.kind != PieceKind::Formula => Some(label),
        _ => None,
    };
    let mut words: Vec<String> = Vec::new();
    words.extend(name.map(|name| format!("**{name}**")));
    words.extend(piece.caption.clone());
    words.push(item.page_label());
    words.join(" \u{2014} ")
}

fn concept_blocks(concepts: &[PageConcept]) -> Vec<Block> {
    if concepts.is_empty() {
        return Vec::new();
    }
    let lines = concepts
        .iter()
        .map(|concept| match concept.definition.as_str() {
            "" => format!("**{}**", concept.name),
            definition => format!("**{}**: {definition}", concept.name),
        })
        .collect();
    vec![Block::List(lines)]
}

impl Shared {
    /// Called when a document opens and when a view is shown. A view that lists pieces, or the
    /// Concepts view, loads the contents of the open document when the slot holds another
    /// document's, or when the part that the view shows failed, so showing the view again tries
    /// again. A part that is ready for this document stays on screen until the new one comes.
    pub(super) fn open_view_contents(&mut self, effects: &mut Vec<Effect>) {
        let Some(doc) = self.shelf.document() else {
            return;
        };
        let is_shown_part_failed = match self.shelf.view {
            DocumentView::Concepts => self.contents.concepts.failure().is_some(),
            view if view.piece_kind().is_some() => self.contents.items.failure().is_some(),
            _ => return,
        };
        let is_same_document = self.contents.is_for(Some(doc));
        if is_same_document && !is_shown_part_failed {
            return;
        }
        let request = self.restart(self.contents.pending, effects);
        self.contents.doc = Some(doc);
        self.contents.pending = Some(request);
        if !(is_same_document && self.contents.items.ready().is_some()) {
            self.contents.items = Loadable::Loading;
        }
        if !(is_same_document && self.contents.concepts.ready().is_some()) {
            self.contents.concepts = Loadable::Loading;
        }
        let folder = self.folder_of(doc);
        effects.push(Effect::Send(Command::LoadChapterContents {
            request,
            doc,
            folder,
        }));
    }

    /// Cancels a pending request and empties the slot. Called when another media or document
    /// opens and when the open document is gone from the catalogue.
    pub(super) fn forget_contents(&mut self, effects: &mut Vec<Effect>) {
        if let Some(request) = self.contents.pending {
            push_cancel(effects, request);
        }
        self.contents = Contents::default();
    }

    /// Lands only for the pending request. The load ends with the concepts, not here.
    pub(super) fn chapter_contents_arrived(
        &mut self,
        request: RequestId,
        result: Result<ChapterContents, Failure>,
    ) {
        if self.contents.pending != Some(request) {
            return;
        }
        self.contents.items = self.settled(result);
    }

    /// Lands only for the pending request, and ends the load.
    pub(super) fn chapter_concepts_arrived(
        &mut self,
        request: RequestId,
        result: Result<Vec<PageConcept>, Failure>,
    ) {
        if self.contents.pending != Some(request) {
            return;
        }
        self.contents.concepts = self.settled(result);
        self.contents.pending = None;
    }

    pub(super) fn contents_is_busy(&self) -> bool {
        self.contents.pending.is_some()
            || self.contents.items.is_loading()
            || self.contents.concepts.is_loading()
    }
}
