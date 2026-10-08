//! Checks that the live backend lists stored documents with their chapters on disk, keeps a media
//! saved before any document between starts, and writes new labels to both stores with no model
//! asked.

mod catalogue;
mod labels;

use std::collections::BTreeSet;
use std::path::Path;

use gui::backend::live::{LiveContext, Services};
use gui::backend::{Handler, Reply};
use gui::contract::{
    Catalogue, Command, DocumentTagsEdit, Event, Failure, MediaEdit, NewMedia, RequestId,
};
use rag_core::MediaLabels;
use rag_ingestion::ChapterFolder;

const REQUEST: RequestId = RequestId(7);

/// The converted chapter in `folder`, whose media is made with no labels when it is new.
fn chapter_at(folder: &Path) -> ChapterFolder<'_> {
    static NO_LABELS: MediaLabels = MediaLabels {
        category: rag_core::Category::Book,
        authors: Vec::new(),
        tags: BTreeSet::new(),
    };
    ChapterFolder {
        folder,
        new_media: &NO_LABELS,
    }
}

fn owned(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_owned()).collect()
}

async fn one_answer<S: Services>(cx: &LiveContext<S>, command: Command) -> Event {
    let (reply, events, _stop) = Reply::collecting();
    cx.serve(command, reply).await;
    let mut sent: Vec<Event> = events.try_iter().collect();
    assert_eq!(sent.len(), 1, "expected one answer: {sent:#?}");
    sent.remove(0)
}

async fn catalogue_of<S: Services>(cx: &LiveContext<S>) -> Result<Catalogue, Failure> {
    let command = Command::LoadCatalogue { request: REQUEST };
    match one_answer(cx, command).await {
        Event::Catalogue {
            request: REQUEST,
            result,
        } => result,
        other => panic!("expected a catalogue, for {REQUEST:?}: {other:#?}"),
    }
}

async fn saved<S: Services>(cx: &LiveContext<S>, media: &NewMedia) -> Result<(), Failure> {
    let command = Command::SaveMedia {
        request: REQUEST,
        media: media.clone(),
    };
    match one_answer(cx, command).await {
        Event::MediaSaved {
            request: REQUEST,
            result,
        } => result,
        other => panic!("expected an answer to the save, for {REQUEST:?}: {other:#?}"),
    }
}

async fn media_edited<S: Services>(cx: &LiveContext<S>, edit: &MediaEdit) -> Result<(), Failure> {
    let command = Command::EditMedia {
        request: REQUEST,
        edit: edit.clone(),
    };
    match one_answer(cx, command).await {
        Event::MediaEdited {
            request: REQUEST,
            result,
        } => result,
        other => panic!("expected an answer to the edit, for {REQUEST:?}: {other:#?}"),
    }
}

async fn document_tags_saved<S: Services>(
    cx: &LiveContext<S>,
    edit: &DocumentTagsEdit,
) -> Result<(), Failure> {
    let command = Command::SetDocumentTags {
        request: REQUEST,
        edit: edit.clone(),
    };
    match one_answer(cx, command).await {
        Event::DocumentTagsSaved {
            request: REQUEST,
            doc,
            result,
        } => {
            assert_eq!(
                doc, edit.doc,
                "the answer names the document that was saved"
            );
            result
        }
        other => panic!("expected an answer to the save, for {REQUEST:?}: {other:#?}"),
    }
}
