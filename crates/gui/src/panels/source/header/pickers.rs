//! The media and document pickers: the lists they choose from, and what they show while the
//! library is not read.

use eframe::egui;

use crate::contract::{ChapterLabel, DocId, DocumentName, Loadable, PageView};
use crate::state::Shared;
use crate::widgets::Dropdown;

/// SMELL: the Library page keeps its own copy of these words for a media with no title, so a
/// change here must also be made there.
const NO_MEDIA: &str = "No media";
const CHOOSE_MEDIA: &str = "Choose a media";
const CHOOSE_DOCUMENT: &str = "Choose a document";

/// The media of the library and the documents of the open document's media, as texts to choose
/// from.
#[derive(Debug, Default)]
pub(super) struct Lists {
    key: Option<(u64, Option<DocId>, Option<DocId>)>,
    media: Vec<String>,
    first_document_ids: Vec<DocId>,
    documents: Vec<String>,
    document_ids: Vec<DocId>,
    /// The places of the open document's media and of the open document in the lists above.
    open_media: Option<usize>,
    open_document: Option<usize>,
    /// What the pickers show while the library is not read: the shown page's own labels.
    page_media: Option<String>,
    page_document: Option<String>,
}

impl Lists {
    pub(super) fn refresh(&mut self, shared: &Shared) {
        let open = shared.source.target.map(|target| target.doc);
        let shown = shared.source.page.ready();
        let key = (shared.library.revision, open, shown.map(|page| page.doc));
        if self.key == Some(key) {
            return;
        }
        *self = Lists {
            key: Some(key),
            page_media: shown.map(media_text),
            page_document: shown.and_then(document_text),
            ..Lists::default()
        };
        let Some(catalogue) = shared.library.catalogue.ready() else {
            return;
        };
        let with_documents = catalogue
            .media
            .iter()
            .filter_map(|media| media.documents.first().map(|first| (media, first.id)));
        for (media, first) in with_documents {
            let is_open = media
                .documents
                .iter()
                .any(|document| Some(document.id) == open);
            if is_open {
                self.open_media = Some(self.media.len());
                for document in &media.documents {
                    if Some(document.id) == open {
                        self.open_document = Some(self.documents.len());
                    }
                    self.documents.push(
                        document
                            .chapter
                            .as_ref()
                            .map_or_else(|| document.title.clone(), chapter_text),
                    );
                    self.document_ids.push(document.id);
                }
            }
            self.media
                .push(media.title.clone().unwrap_or_else(|| NO_MEDIA.to_owned()));
            self.first_document_ids.push(first);
        }
    }

    pub(super) fn pickers<'a>(&'a self, shared: &'a Shared) -> [Picker<'a>; 2] {
        let catalogue = &shared.library.catalogue;
        let is_read = catalogue.ready().is_some();
        let why_off = match catalogue {
            Loadable::Ready(_) => None,
            Loadable::Failed(failure) => Some(failure.hint.as_str()),
            Loadable::Idle | Loadable::Loading => Some("Reading the library…"),
        };
        let (media, document) = if is_read {
            (CHOOSE_MEDIA, CHOOSE_DOCUMENT)
        } else {
            (
                self.page_media.as_deref().unwrap_or(CHOOSE_MEDIA),
                self.page_document.as_deref().unwrap_or(CHOOSE_DOCUMENT),
            )
        };
        [
            Picker {
                salt: "media",
                label: "Media",
                options: &self.media,
                opens: &self.first_document_ids,
                selected: self.open_media,
                placeholder: media,
                is_enabled: is_read && !self.media.is_empty(),
                why_off,
            },
            Picker {
                salt: "document",
                label: "Document",
                options: &self.documents,
                opens: &self.document_ids,
                selected: self.open_document,
                placeholder: document,
                is_enabled: is_read && !self.documents.is_empty(),
                why_off,
            },
        ]
    }
}

fn media_text(page: &PageView) -> String {
    page.media.clone().unwrap_or_else(|| NO_MEDIA.to_owned())
}

fn document_text(page: &PageView) -> Option<String> {
    let chapter = page.chapter.as_ref().map(chapter_text);
    chapter.or_else(|| page.document_title.clone())
}

/// A chapter reads the same here as everywhere else in the app.
fn chapter_text(chapter: &ChapterLabel) -> String {
    DocumentName::Chapter {
        number: chapter.number,
        name: chapter.name.clone(),
    }
    .label()
}

#[derive(Debug)]
pub(super) struct Picker<'a> {
    salt: &'a str,
    label: &'a str,
    options: &'a [String],
    /// The document that each option opens.
    opens: &'a [DocId],
    selected: Option<usize>,
    placeholder: &'a str,
    is_enabled: bool,
    why_off: Option<&'a str>,
}

impl Picker<'_> {
    pub(super) fn show(&self, ui: &mut egui::Ui, width: f32) -> Option<DocId> {
        let shown = ui.add_enabled_ui(self.is_enabled, |ui| {
            Dropdown::new(self.label, self.options)
                .id_salt(self.salt)
                .selected(self.selected)
                .placeholder(self.placeholder)
                .width(width)
                .show(ui)
        });
        if let Some(why_off) = self.why_off {
            shown.response.on_disabled_hover_text(why_off);
        }
        let chosen = shown.inner.filter(|place| Some(*place) != self.selected)?;
        self.opens.get(chosen).copied()
    }
}
