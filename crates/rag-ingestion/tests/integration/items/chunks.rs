//! How text and footnote pieces are packed into chunks: the section and the token limit, the
//! paragraph that a page break cut, and the footnote that joins its paragraph or stands alone.

use ocr::{PieceDetail, read_chapter};
use rag_core::ItemKind;
use rag_ingestion::{Item, chapter_items};

use super::{content_of, of_kind};
use crate::support;

fn estimated_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

#[test]
fn chunks_stay_inside_one_section_and_inside_the_token_limit() {
    let chapter = read_chapter(&support::sample_chapter()).unwrap();
    let items = chapter_items(&chapter);
    let chunks = of_kind(&items, ItemKind::Chunk);

    let delta: Vec<&&Item> = chunks
        .iter()
        .filter(|chunk| chunk.payload.text.contains(content_of(&chapter, 3, 5)))
        .collect();
    assert_eq!(delta.len(), 1);
    for neighbour in [content_of(&chapter, 3, 3), content_of(&chapter, 3, 7)] {
        assert!(
            !delta[0].payload.text.contains(neighbour),
            "the section of the delta shares a chunk with {neighbour}"
        );
    }

    let first_section_chunks = chunks
        .iter()
        .filter(|chunk| chunk.payload.page == 1)
        .count();
    assert!(
        first_section_chunks > 1,
        "five paragraphs under one heading are more than 500 tokens, so they are split"
    );

    for piece in chapter
        .pieces
        .iter()
        .filter(|piece| matches!(piece.detail, PieceDetail::Text { .. }))
    {
        let holders = chunks
            .iter()
            .filter(|chunk| chunk.payload.text.contains(&piece.content))
            .count();
        assert_eq!(
            holders, 1,
            "text piece {:?} is in {holders} chunks",
            piece.id
        );
    }
    for chunk in &chunks {
        assert!(estimated_tokens(&chunk.input.text) <= 500);
    }
}

#[test]
fn a_paragraph_split_by_a_page_break_is_rejoined_only_when_both_flags_are_set() {
    let chapter = read_chapter(&support::sample_chapter()).unwrap();
    let items = chapter_items(&chapter);
    let chunks = of_kind(&items, ItemKind::Chunk);
    let joined_by_a_space = |(first_page, first_number), (second_page, second_number)| {
        let joined = format!(
            "{} {}",
            content_of(&chapter, first_page, first_number),
            content_of(&chapter, second_page, second_number)
        );
        chunks
            .iter()
            .any(|chunk| chunk.payload.text.contains(&joined))
    };

    assert!(
        joined_by_a_space((6, 9), (7, 1)),
        "page 6 ends and page 7 starts in the middle of one sentence"
    );
    assert!(
        !joined_by_a_space((2, 7), (3, 3)),
        "page 3 starts mid-sentence but page 2 does not end that way"
    );
    assert!(
        !joined_by_a_space((4, 9), (5, 2)),
        "page 4 ends mid-sentence but page 5 does not start that way"
    );
}

#[test]
fn a_footnote_joins_the_chunk_of_its_paragraph_or_stands_alone() {
    let sample = read_chapter(&support::sample_chapter()).unwrap();
    let sample_items = chapter_items(&sample);
    let footnote = content_of(&sample, 2, 9);
    let holders: Vec<&Item> = sample_items
        .iter()
        .filter(|item| item.payload.text.contains(footnote))
        .collect();
    assert_eq!(holders.len(), 1, "the footnote is in exactly one item");
    assert_eq!(holders[0].payload.kind, ItemKind::Chunk);
    assert!(
        holders[0].payload.text.contains(content_of(&sample, 2, 6)),
        "the paragraph that carries the marker is in the same chunk"
    );
    assert!(
        holders[0]
            .payload
            .text
            .contains(&format!("[^1]: {footnote}"))
    );

    let intuition = read_chapter(&support::intuition_chapter()).unwrap();
    let intuition_items = chapter_items(&intuition);
    let table_footnote = content_of(&intuition, 2, 6);
    let alone: Vec<&Item> = intuition_items
        .iter()
        .filter(|item| item.payload.text.contains(table_footnote))
        .collect();
    assert_eq!(alone.len(), 1);
    assert_eq!(alone[0].payload.kind, ItemKind::Chunk);
    assert_eq!(
        alone[0].payload.text, table_footnote,
        "a footnote of a table has no paragraph to join, so it is a chunk by itself"
    );
}
