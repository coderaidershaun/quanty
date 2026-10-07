//! The one item of a picture that stands alone: a figure on page 1 of a document that is named
//! after the picture's file.

use ocr::ConvertedImage;
use rag_core::{DocId, DocumentInput, ItemId, ItemKind, ItemPayload};

use super::pieces::{join_blocks, label_line};
use super::{BLOCK_SEPARATOR, Item};

/// A picture that stands alone, and what the person who added it said about it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoneImage<'a> {
    pub image: &'a ConvertedImage,
    pub note: Option<&'a str>,
}

/// The one item of a picture that stands alone. The document id is made from the bytes of the
/// picture, so the note does not change it. The note is added to the explanation, so it is
/// embedded with the picture and is read when the concepts are asked for.
pub fn image_items(picture: &LoneImage<'_>) -> Vec<Item> {
    let image = picture.image;
    let document = DocId::from_source_sha256(&image.index.source_sha256);
    let doc_title = image.index.source_file.clone();
    let explanation = match picture.note.map(str::trim) {
        Some(note) if !note.is_empty() => {
            format!("{}{BLOCK_SEPARATOR}Note: {note}", image.explanation)
        }
        _ => image.explanation.clone(),
    };
    let heading = label_line(image.index.label.as_deref(), image.index.caption.as_deref());
    let text = join_blocks([heading.as_deref(), Some(&explanation)]);
    vec![Item {
        id: ItemId::new(document, ItemKind::Figure, 0),
        payload: ItemPayload {
            doc_id: document,
            doc_title: doc_title.clone(),
            page: 1,
            printed_page: None,
            kind: ItemKind::Figure,
            text: explanation,
            image_path: Some(image.picture.clone()),
        },
        input: DocumentInput {
            title: doc_title,
            text,
            image: Some(image.picture.clone()),
        },
    }]
}
