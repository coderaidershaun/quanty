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

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::super::{Contents, Insights};
    use super::*;
    use contract::{Command, Document, Event, Failure, Intent, Media, RequestId};

    fn doc(number: u128) -> DocId {
        DocId(Uuid::from_u128(number))
    }

    fn media(title: &str, documents: &[DocId]) -> Media {
        Media {
            title: Some(title.to_owned()),
            documents: documents
                .iter()
                .map(|id| Document {
                    id: *id,
                    ..Document::default()
                })
                .collect(),
            ..Media::default()
        }
    }

    fn alpha() -> Media {
        media("Alpha", &[doc(1), doc(2)])
    }

    fn beta() -> Media {
        media("Beta", &[doc(3)])
    }

    fn open_media_titled(title: &str) -> Option<Open> {
        Some(Open::Media {
            title: Some(title.to_owned()),
        })
    }

    /// Reads the library as the app does: a refresh goes out and the catalogue answers it.
    /// Returns what the arrival asked for.
    fn read(shared: &mut Shared, result: Result<Catalogue, Failure>) -> Vec<Effect> {
        shared.apply_intent(Intent::RefreshCatalogue, &mut Vec::new());
        let request = shared.library.pending.expect("a refresh is on its way");
        let mut effects = Vec::new();
        shared.apply_event(Event::Catalogue { request, result }, &mut effects);
        effects
    }

    fn cancels(requests: [u64; 2]) -> Vec<Effect> {
        requests
            .map(|request| Effect::Send(Command::Cancel(RequestId(request))))
            .to_vec()
    }

    #[test]
    fn the_first_media_opens_when_a_catalogue_arrives_and_nothing_is_open() {
        let mut shared = Shared::default();
        read(&mut shared, Err(Failure::not_built("the catalogue")));
        assert_eq!(
            shared.shelf.open, None,
            "a catalogue that failed opens nothing"
        );

        read(&mut shared, Ok(Catalogue::default()));
        assert_eq!(
            shared.shelf.open, None,
            "a library with no media has nothing to open"
        );

        read(
            &mut shared,
            Ok(Catalogue {
                media: vec![alpha(), beta()],
            }),
        );
        assert_eq!(shared.shelf.open, open_media_titled("Alpha"));
        assert_eq!(
            shared.tab,
            Tab::Ask,
            "a catalogue never pulls the window to the Library"
        );

        shared.apply_intent(Intent::OpenDocument(doc(3)), &mut Vec::new());
        read(
            &mut shared,
            Ok(Catalogue {
                media: vec![alpha(), beta()],
            }),
        );
        assert_eq!(
            shared.shelf.open,
            Some(Open::Document(doc(3))),
            "an item that is still listed stays open"
        );
    }

    #[test]
    fn an_open_document_that_is_gone_falls_back_to_its_media_and_a_gone_media_to_the_first() {
        let mut shared = Shared::default();
        read(
            &mut shared,
            Ok(Catalogue {
                media: vec![alpha(), beta()],
            }),
        );

        shared.apply_intent(Intent::OpenDocument(doc(2)), &mut Vec::new());
        shared.contents.pending = Some(RequestId(90));
        shared.insights.pending = Some(RequestId(91));
        let only_the_first = media("Alpha", &[doc(1)]);
        let effects = read(
            &mut shared,
            Ok(Catalogue {
                media: vec![only_the_first, beta()],
            }),
        );
        assert_eq!(shared.shelf.open, open_media_titled("Alpha"));
        assert_eq!(
            effects,
            cancels([90, 91]),
            "the loads for the gone document stop"
        );
        assert_eq!(shared.contents, Contents::default());
        assert_eq!(shared.insights, Insights::default());

        shared.apply_intent(
            Intent::OpenMedia {
                title: Some("Beta".to_owned()),
            },
            &mut Vec::new(),
        );
        read(
            &mut shared,
            Ok(Catalogue {
                media: vec![alpha()],
            }),
        );
        assert_eq!(
            shared.shelf.open,
            open_media_titled("Alpha"),
            "a gone media falls back to the first"
        );

        read(
            &mut shared,
            Ok(Catalogue {
                media: vec![alpha(), beta()],
            }),
        );
        shared.apply_intent(Intent::OpenDocument(doc(3)), &mut Vec::new());
        read(
            &mut shared,
            Ok(Catalogue {
                media: vec![alpha()],
            }),
        );
        assert_eq!(
            shared.shelf.open,
            open_media_titled("Alpha"),
            "a document of a gone media falls back to the first media"
        );
    }

    #[test]
    fn opening_another_item_forgets_the_contents_and_the_insights_and_keeps_the_view() {
        let is_load_of = |effects: &[Effect], wanted: DocId| match effects {
            [Effect::Send(Command::LoadChapterContents { doc, .. })] => *doc == wanted,
            _ => false,
        };
        let mut shared = Shared::default();
        let mut effects = Vec::new();
        shared.apply_intent(Intent::OpenDocument(doc(1)), &mut effects);
        shared.apply_intent(Intent::ShowDocumentView(DocumentView::Code), &mut effects);
        assert!(
            is_load_of(&effects, doc(1)),
            "the Code view loads the open document: {effects:?}"
        );
        let first = shared
            .contents
            .pending
            .expect("the contents are on their way");
        shared.insights.pending = Some(RequestId(91));
        shared.insights.doc = Some(doc(1));
        effects.clear();

        shared.apply_intent(Intent::OpenDocument(doc(2)), &mut effects);
        assert_eq!(effects[..2], cancels([first.0, 91])[..]);
        assert!(
            is_load_of(&effects[2..], doc(2)),
            "the view that stays loads the new document: {effects:?}"
        );
        assert_eq!(shared.contents.doc, Some(doc(2)));
        assert_eq!(shared.insights, Insights::default());
        assert_eq!(shared.shelf.view, DocumentView::Code);
        assert_eq!(shared.tab, Tab::Library);

        effects.clear();
        let pending = shared.contents.pending;
        shared.apply_intent(Intent::OpenDocument(doc(2)), &mut effects);
        assert!(
            effects.is_empty(),
            "the same document again forgets nothing"
        );
        assert_eq!(shared.contents.pending, pending);
    }
}
