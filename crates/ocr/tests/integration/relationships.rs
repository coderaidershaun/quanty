//! Checks on the relationships of the stubbed chapter run: those saved on one page, and the
//! lead-in that only reading the chapter back finds across a page break.

use std::path::Path;

use ocr::ConversionSummary;
use ocr::content::RelationshipKind;
use ocr::reader::{PieceId, read_chapter};
use ocr::testing::{page_folder, read_json};

pub fn assert_relationships_saved(chapter: &Path) {
    let page_two = read_json(&page_folder(chapter, 2).join("page.json"));
    let relationships: Vec<(&str, u64, u64)> = page_two["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .map(|link| {
            (
                link["kind"].as_str().unwrap(),
                link["from"].as_u64().unwrap(),
                link["to"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        relationships,
        [
            ("introduces", 2, 3),
            ("discusses", 4, 5),
            ("footnote-of", 7, 4)
        ]
    );
    let cites = page_two["pieces"][3]["cites"].as_array().unwrap();
    assert!(
        cites
            .iter()
            .any(|cite| cite["kind"] == "footnote" && cite["label"] == "1")
    );
}

/// The lead-in of the formula that opens page 7 is found on page 6, though no saved
/// relationship crosses a page.
pub fn assert_reads_back_with_bridged_lead_in(chapter: &Path, summary: &ConversionSummary) {
    let read_back = read_chapter(chapter).unwrap();
    let saved_pieces = summary.pieces;
    assert_eq!(
        read_back.pieces.len() as u32,
        saved_pieces.heading
            + saved_pieces.text
            + saved_pieces.formula
            + saved_pieces.figure
            + saved_pieces.table
            + saved_pieces.footnote
    );
    let page_seven = read_json(&page_folder(chapter, 7).join("page.json"));
    assert!(page_seven["relationships"].as_array().unwrap().is_empty());
    let lead_in = PieceId { page: 6, number: 4 };
    let formula = PieceId { page: 7, number: 1 };
    for end in [lead_in, formula] {
        let piece = read_back.piece(end).unwrap();
        assert!(
            piece
                .relationships
                .iter()
                .any(|link| link.kind == RelationshipKind::Introduces
                    && link.from == lead_in
                    && link.to == formula),
            "{end:?} lacks the bridged edge"
        );
    }
}
