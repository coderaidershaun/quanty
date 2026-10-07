//! The sample chapter and the two authored chapters, mapped whole: identifiers, reading order,
//! what a formula, a figure and a table store and give the embedder, and labels and cites.

use std::collections::BTreeSet;

use ocr::{Chapter, ChapterPiece, CiteKind, PieceDetail, RelationshipKind, read_chapter};
use rag_core::{DocId, ItemId, ItemKind};
use rag_ingestion::{Item, chapter_items};

use super::{content_of, of_kind};
use crate::support;

fn pieces_of(chapter: &Chapter, wanted: fn(&PieceDetail) -> bool) -> Vec<&ChapterPiece> {
    chapter
        .pieces
        .iter()
        .filter(|piece| wanted(&piece.detail))
        .collect()
}

fn is_formula(detail: &PieceDetail) -> bool {
    matches!(detail, PieceDetail::Formula { .. })
}

fn is_figure(detail: &PieceDetail) -> bool {
    matches!(detail, PieceDetail::Figure { .. })
}

fn is_table(detail: &PieceDetail) -> bool {
    matches!(detail, PieceDetail::Table { .. })
}

fn is_heading(detail: &PieceDetail) -> bool {
    matches!(detail, PieceDetail::Heading { .. })
}

const SAMPLE_TITLE: &str = "Option Volatility and Pricing, chapter 1: Sample Pages";

/// The text piece that leads into the formula: the latest of those that introduce it.
fn lead_in_of<'a>(chapter: &'a Chapter, formula: &ChapterPiece) -> Option<&'a ChapterPiece> {
    formula
        .relationships
        .iter()
        .filter(|link| link.kind == RelationshipKind::Introduces && link.to == formula.id)
        .map(|link| link.from)
        .max()
        .and_then(|id| chapter.piece(id))
}

#[test]
fn sample_chapter_maps_to_items_of_all_four_kinds_in_reading_order() {
    let chapter = read_chapter(&support::sample_chapter()).unwrap();
    let items = chapter_items(&chapter);

    let formulas = of_kind(&items, ItemKind::Formula);
    let figures = of_kind(&items, ItemKind::Figure);
    let tables = of_kind(&items, ItemKind::Table);
    let chunks = of_kind(&items, ItemKind::Chunk);
    assert_eq!(
        (formulas.len(), figures.len(), tables.len()),
        (5, 3, 3),
        "one item for each formula, figure and table piece"
    );
    assert!(!chunks.is_empty());
    assert_eq!(
        items.len(),
        formulas.len() + figures.len() + tables.len() + chunks.len()
    );
    for heading in pieces_of(&chapter, is_heading) {
        assert!(
            items
                .iter()
                .all(|item| item.payload.text != heading.content),
            "a heading is context, not an item: {}",
            heading.content
        );
    }
    assert_eq!(
        items,
        chapter_items(&chapter),
        "mapping twice gives the same items"
    );

    assert_identifiers_follow_the_stored_scheme(&chapter, &items);
    assert_items_are_in_reading_order(&chapter, &items);
    assert_formulas_store_raw_latex_and_embed_their_lead_in(&chapter, &formulas);
    assert_tables_embed_their_summary_and_body(&chapter, &tables);
    assert_figures_carry_their_own_picture(&chapter, &figures);
    assert_items_keep_both_page_numbers_and_their_section(&items);
    assert_labels_and_cites_follow_the_pieces(
        &chapter,
        &items,
        &[
            "Figure 7-2",
            "Figure 7-3",
            "(7.3)",
            "(7.4)",
            "Figure 13-4",
            "Figure 13-16",
            "Figure 24-12",
        ],
        &[
            "(4.18)",
            "(4.20)",
            "(7.2)",
            "Figure 13-3",
            "Figure 24-12",
            "Figure 24-13",
        ],
    );
}

fn assert_identifiers_follow_the_stored_scheme(chapter: &Chapter, items: &[Item]) {
    let document = DocId::from_source_sha256(&chapter.index.source_sha256);
    assert_ne!(document, DocId::from_source_sha256("another source"));
    for (position, item) in items.iter().enumerate() {
        let position = u32::try_from(position).unwrap();
        assert_eq!(item.id, ItemId::new(document, item.payload.kind, position));
        assert_eq!(item.payload.doc_id, document);
        assert_eq!(item.payload.doc_title, SAMPLE_TITLE);
        assert_eq!(item.id.to_string().as_bytes()[14], b'5', "a version 5 UUID");
    }

    // Do not change these two values to make the test pass. Every stored point is named this
    // way, so code that gives other values no longer finds the points that are already stored.
    let pinned_document = DocId::from_source_sha256(
        "b37ff5181567b3ea99c0ec2b7aca436c036a127477f34dbe9299c8ab13cf0516",
    );
    assert_eq!(
        pinned_document.to_string(),
        "48a73bfc-835d-5780-bb33-339fef634a92"
    );
    assert_eq!(
        ItemId::new(pinned_document, ItemKind::Chunk, 0).to_string(),
        "663a03c0-23a3-534c-8564-9f9ff209d487"
    );
}

/// By page, and a chunk comes before the formulas that sit inside the text it holds.
fn assert_items_are_in_reading_order(chapter: &Chapter, items: &[Item]) {
    assert!(
        items
            .windows(2)
            .all(|pair| pair[0].payload.page <= pair[1].payload.page)
    );
    let first_text_of_page_4 = content_of(chapter, 4, 1);
    let chunk_position = items
        .iter()
        .position(|item| item.payload.text.contains(first_text_of_page_4))
        .unwrap();
    let formula_position = items
        .iter()
        .position(|item| item.payload.kind == ItemKind::Formula && item.payload.page == 4)
        .unwrap();
    assert!(chunk_position < formula_position);
}

fn assert_formulas_store_raw_latex_and_embed_their_lead_in(chapter: &Chapter, formulas: &[&Item]) {
    for (item, piece) in formulas.iter().zip(pieces_of(chapter, is_formula)) {
        assert_eq!(item.payload.text, piece.content, "the raw LaTeX is stored");
        let PieceDetail::Formula { statement, .. } = &piece.detail else {
            unreachable!("filtered to formulas")
        };
        assert!(item.input.text.contains(&piece.content));
        assert!(item.input.text.contains(statement.as_str()));
        let lead_in =
            lead_in_of(chapter, piece).expect("every formula of the sample has a lead-in");
        assert!(item.input.text.contains(&lead_in.content));
        assert!(item.input.image.is_none() && item.payload.image_path.is_none());
    }
}

fn assert_tables_embed_their_summary_and_body(chapter: &Chapter, tables: &[&Item]) {
    for (item, piece) in tables.iter().zip(pieces_of(chapter, is_table)) {
        let PieceDetail::Table { summary, .. } = &piece.detail else {
            unreachable!("filtered to tables")
        };
        assert_eq!(item.payload.text, piece.content);
        assert!(item.input.text.contains(summary.as_str()));
        assert!(item.input.text.contains(&piece.content));
    }
}

/// The picture is embedded together with the explanation, and its path is kept in the payload.
fn assert_figures_carry_their_own_picture(chapter: &Chapter, figures: &[&Item]) {
    for (item, piece) in figures.iter().zip(pieces_of(chapter, is_figure)) {
        let picture = &piece.figure_image.as_ref().unwrap().path;
        assert!(picture.to_string_lossy().ends_with("-figure.png"));
        assert!(picture.is_file());
        assert_eq!(item.input.image.as_ref(), Some(picture));
        assert_eq!(item.payload.image_path.as_ref(), Some(picture));
        assert_eq!(item.payload.text, piece.content);
        assert!(item.input.text.contains(&piece.content));
    }
}

fn assert_items_keep_both_page_numbers_and_their_section(items: &[Item]) {
    let formula_of_page_4 = items
        .iter()
        .find(|item| item.payload.kind == ItemKind::Formula && item.payload.page == 4)
        .unwrap();
    assert_eq!(
        formula_of_page_4.payload.printed_page.as_deref(),
        Some("147")
    );
    assert!(items.iter().all(|item| item.payload.page >= 1));
    assert!(items.iter().all(|item| item.payload.printed_page.is_some()));
    assert!(
        items
            .iter()
            .all(|item| item.input.title.starts_with(SAMPLE_TITLE))
    );
    let delta = items
        .iter()
        .find(|item| item.payload.text.contains("The *delta*"))
        .unwrap();
    assert!(
        delta.input.title.contains("The Delta"),
        "{}",
        delta.input.title
    );
}

/// A formula, a figure or a table keeps the label it is printed with, and a chunk keeps the
/// labels of the figures, tables and equations that its text points at, each once. A footnote
/// marker is not one of them. `printed_labels` are the labels of the items that have one, and
/// `cited_labels` those of the chunks, each list in reading order.
fn assert_labels_and_cites_follow_the_pieces(
    chapter: &Chapter,
    items: &[Item],
    printed_labels: &[&str],
    cited_labels: &[&str],
) {
    let labelled: Vec<&str> = items
        .iter()
        .filter_map(|item| item.payload.label.as_deref())
        .collect();
    assert_eq!(labelled, printed_labels);
    for item in items {
        match item.payload.kind {
            ItemKind::Chunk => assert!(item.payload.label.is_none()),
            _ => assert!(item.payload.cites.is_empty()),
        }
    }

    let chunks = of_kind(items, ItemKind::Chunk);
    let cited: Vec<&str> = chunks
        .iter()
        .flat_map(|chunk| chunk.payload.cites.iter().map(String::as_str))
        .collect();
    assert_eq!(cited, cited_labels);
    for chunk in &chunks {
        let once: BTreeSet<&String> = chunk.payload.cites.iter().collect();
        assert_eq!(
            once.len(),
            chunk.payload.cites.len(),
            "{:?}",
            chunk.payload.cites
        );
    }
    for piece in &chapter.pieces {
        let PieceDetail::Text { cites } = &piece.detail else {
            continue;
        };
        let holder = chunks
            .iter()
            .find(|chunk| chunk.payload.text.contains(&piece.content))
            .expect("every text piece is in a chunk");
        for cite in cites.iter().filter(|cite| cite.kind != CiteKind::Footnote) {
            assert!(
                holder.payload.cites.contains(&cite.label),
                "the chunk of {:?} does not keep {}",
                piece.id,
                cite.label
            );
        }
    }
}

#[test]
fn authored_chapters_read_back_and_the_in_depth_one_gives_the_black_scholes_formulas() {
    let sample = read_chapter(&support::sample_chapter()).unwrap();
    let intuition = read_chapter(&support::intuition_chapter()).unwrap();
    let in_depth = read_chapter(&support::in_depth_chapter()).unwrap();

    let hashes = [
        &sample.index.source_sha256,
        &intuition.index.source_sha256,
        &in_depth.index.source_sha256,
    ];
    assert!(hashes[0] != hashes[1] && hashes[0] != hashes[2] && hashes[1] != hashes[2]);
    assert_eq!(intuition.index.chapter_number, 1);
    assert_eq!(in_depth.index.chapter_number, 2);

    let intuition_items = chapter_items(&intuition);
    let in_depth_items = chapter_items(&in_depth);
    let in_depth_formulas = of_kind(&in_depth_items, ItemKind::Formula);
    let formula_with = |parts: &[&str]| {
        in_depth_formulas
            .iter()
            .find(|item| parts.iter().all(|part| item.payload.text.contains(part)))
            .copied()
    };
    let partial_differential_equation =
        formula_with(&["- r V = 0"]).expect("the Black-Scholes equation is a formula item");
    assert!(
        formula_with(&["N(d_1)", "N(d_2)"]).is_some(),
        "the call price"
    );
    assert!(formula_with(&["N(-d_1)"]).is_some(), "the put price");
    assert!(
        partial_differential_equation
            .input
            .text
            .contains(content_of(&in_depth, 2, 6)),
        "the equation opens its page, so its lead-in is the last text of the page before"
    );

    for (chapter, items) in [(&intuition, &intuition_items), (&in_depth, &in_depth_items)] {
        let formulas = of_kind(items, ItemKind::Formula);
        let pieces = pieces_of(chapter, is_formula);
        assert_eq!(formulas.len(), pieces.len());
        for (item, piece) in formulas.iter().zip(pieces) {
            let lead_in = lead_in_of(chapter, piece)
                .unwrap_or_else(|| panic!("formula {:?} has no lead-in", piece.id));
            assert!(item.input.text.contains(&lead_in.content), "{:?}", piece.id);
        }
    }

    assert!(
        of_kind(&intuition_items, ItemKind::Chunk)
            .iter()
            .any(|chunk| chunk.payload.text.contains("Black–Scholes"))
    );

    assert_labels_and_cites_follow_the_pieces(
        &intuition,
        &intuition_items,
        &["(1.1)", "(1.2)", "Table 1-1"],
        &["Table 1-1"],
    );
    assert_labels_and_cites_follow_the_pieces(
        &in_depth,
        &in_depth_items,
        &["(2.1)", "(2.2)", "(2.3)", "(2.4)", "(2.5)", "(2.6)"],
        &["(2.4)", "(2.5)"],
    );
}
