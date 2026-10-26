//! What the Library tab has open: a media, or one of its documents in one of its views, and the
//! rules that keep the open item on the shelf when the catalogue changes.

use super::shared::Shared;
use contract::{Catalogue, DocId, DocumentView, Effect, Tab};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Open {
    /// `None` is the group of documents that belong to no media.
    Media {
        title: Option<String>,
    },
    Document(DocId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Shelf {
    pub open: Option<Open>,
    /// Kept when another document opens, so a person can read one view of chapter after chapter.
    pub view: DocumentView,
}

impl Shelf {
    /// The open document, when a document is open.
    pub fn document(&self) -> Option<DocId> {
        match self.open {
            Some(Open::Document(doc)) => Some(doc),
            Some(Open::Media { .. }) | None => None,
        }
    }
}

impl Open {
    /// A media is listed by its exact title, and a document by its id.
    fn is_listed_in(&self, catalogue: &Catalogue) -> bool {
        match self {
            Open::Media { title } => is_media_listed(title.as_deref(), catalogue),
            Open::Document(doc) => catalogue.document(*doc).is_some(),
        }
    }
}

fn is_media_listed(title: Option<&str>, catalogue: &Catalogue) -> bool {
    catalogue
        .media
        .iter()
        .any(|media| media.title.as_deref() == title)
}

// The intents set the tab themselves and do not use `open_tab`, so a maximised panel of Ask stays
// maximised, as it does when a source is opened.
impl Shared {
    pub(super) fn open_media(&mut self, title: Option<String>, effects: &mut Vec<Effect>) {
        self.tab = Tab::Library;
        self.open_item(Open::Media { title }, effects);
    }

    pub(super) fn open_document(&mut self, doc: DocId, effects: &mut Vec<Effect>) {
        self.tab = Tab::Library;
        self.open_item(Open::Document(doc), effects);
        self.open_view_contents(effects);
        self.open_view_insights(effects);
    }

    pub(super) fn show_document_view(&mut self, view: DocumentView, effects: &mut Vec<Effect>) {
        self.shelf.view = view;
        self.open_view_contents(effects);
        self.open_view_insights(effects);
    }

    /// The slots of the views belong to one document, so another item empties them.
    fn open_item(&mut self, item: Open, effects: &mut Vec<Effect>) {
        if self.shelf.open.as_ref() == Some(&item) {
            return;
        }
        self.forget_contents(effects);
        self.forget_insights(effects);
        self.shelf.open = Some(item);
    }

    /// Called when a catalogue has been read. `fallback` is the media that the open document
    /// belonged to in the catalogue before this one. An open item that is listed stays open, and
    /// one that is not falls back to its media, or else to the first media.
    ///
    /// It never changes the tab: a catalogue that arrives while another tab is open must not pull
    /// the window onto the Library.
    // SMELL: in `fallback` the outer `None` means no fallback and the inner `None` means the group
    // with no media, which a reader must work out. A small enum would name both.
    pub(super) fn keep_shelf_on_listed_items(
        &mut self,
        fallback: Option<Option<String>>,
        effects: &mut Vec<Effect>,
    ) {
        let Some(catalogue) = self.library.catalogue.ready() else {
            return;
        };
        if self
            .shelf
            .open
            .as_ref()
            .is_some_and(|open| open.is_listed_in(catalogue))
        {
            return;
        }
        let title = match fallback {
            Some(title) if is_media_listed(title.as_deref(), catalogue) => Some(title),
            _ => catalogue.media.first().map(|media| media.title.clone()),
        };
        self.forget_contents(effects);
        self.forget_insights(effects);
        self.shelf.open = title.map(|title| Open::Media { title });
    }
}
