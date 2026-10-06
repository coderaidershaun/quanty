//! Reads the committed sample chapter back, and holds the checks about it that a fresh live
//! conversion must also pass.

use std::path::{Path, PathBuf};

use converter::content::{CiteKind, PieceDetail, RelationshipKind};
use converter::reader::{Chapter, ChapterPiece, PieceId, read_chapter};

const SOFT_HYPHEN: char = '\u{AD}';

fn committed_chapter() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/content/option-volatility-and-pricing/chapter-1")
}

fn on_page(chapter: &Chapter, page: u32) -> impl Iterator<Item = &ChapterPiece> {
    chapter
        .pieces
        .iter()
        .filter(move |piece| piece.id.page == page)
}

fn label_of(piece: &ChapterPiece) -> Option<&str> {
    match &piece.detail {
        PieceDetail::Formula { label, .. }
        | PieceDetail::Figure { label, .. }
        | PieceDetail::Table { label, .. } => label.as_deref(),
        _ => None,
    }
}

fn cites(piece: &ChapterPiece) -> Vec<(CiteKind, &str)> {
    match &piece.detail {
        PieceDetail::Text { cites } | PieceDetail::Footnote { cites, .. } => cites
            .iter()
            .map(|cite| (cite.kind, cite.label.as_str()))
            .collect(),
        _ => Vec::new(),
    }
}

fn is_text(chapter: &Chapter, id: PieceId) -> bool {
    matches!(
        chapter.piece(id).map(|piece| &piece.detail),
        Some(PieceDetail::Text { .. })
    )
}

fn find_labelled<'a>(chapter: &'a Chapter, page: u32, label: &str) -> &'a ChapterPiece {
    on_page(chapter, page)
        .find(|piece| label_of(piece) == Some(label))
        .unwrap_or_else(|| panic!("page {page} has no piece labelled {label}"))
}

/// The header cells and the data rows of a table piece's Markdown, leaving out any note under it.
fn table_shape(piece: &ChapterPiece) -> (usize, usize) {
    let lines: Vec<&str> = piece
        .content
        .lines()
        .take_while(|line| !line.trim().is_empty())
        .collect();
    assert!(
        lines[1].chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) && lines[1].contains('-'),
        "no separator row in {}",
        piece.content
    );
    let cells = lines[0].trim_matches('|').split('|').count();
    (cells, lines.len() - 2)
}

/// What must be true of the seven sample pages however the model words things, and whichever
/// model wrote the plain text page.
pub fn assert_sample_chapter(chapter_folder: &Path) {
    let chapter = read_chapter(chapter_folder).unwrap();
    assert_reading_order_and_page_numbers(&chapter);
    assert_no_soft_hyphen_saved(&chapter, chapter_folder);
    assert_exact_words_where_the_text_layer_is_wrong(&chapter);
    assert_sections_carried_across_pages(&chapter);
    assert_math_page(&chapter);
    assert_chart_page(&chapter);
    assert_tables_page(&chapter);
    assert_footnote_page(&chapter);
    assert_volatility_surface_page(&chapter);
    assert_mixed_page(&chapter);
}

fn assert_reading_order_and_page_numbers(chapter: &Chapter) {
    let ids: Vec<PieceId> = chapter.pieces.iter().map(|piece| piece.id).collect();
    assert!(
        ids.windows(2).all(|pair| pair[0] < pair[1]),
        "pieces are out of order"
    );
    let printed: Vec<String> = (1..=7)
        .map(|page| {
            on_page(chapter, page)
                .next()
                .and_then(|piece| piece.printed_page_number.clone())
                .unwrap_or_else(|| panic!("page {page} has no printed page number"))
        })
        .collect();
    assert_eq!(printed, ["1", "12", "100", "147", "233", "246", "500"]);
}

/// The text layer holds this character by design, so only the saved index and piece files are
/// searched.
fn assert_no_soft_hyphen_saved(chapter: &Chapter, chapter_folder: &Path) {
    for page in 1..=7 {
        let page_json = chapter_folder.join(format!("page-num-{page}/page.json"));
        assert!(
            !std::fs::read_to_string(&page_json)
                .unwrap()
                .contains(SOFT_HYPHEN)
        );
    }
    assert!(
        chapter
            .pieces
            .iter()
            .all(|piece| !std::fs::read_to_string(&piece.file)
                .unwrap()
                .contains(SOFT_HYPHEN)),
        "an invisible soft hyphen was saved"
    );
}

fn assert_exact_words_where_the_text_layer_is_wrong(chapter: &Chapter) {
    for exact in [
        "the May 46 and May 50 puts",
        "May 52 call",
        "(the theta)",
        "friend Jerry",
        "fair one-year forward price",
    ] {
        assert!(
            chapter
                .pieces
                .iter()
                .any(|piece| piece.content.contains(exact)),
            "no piece holds {exact:?}"
        );
    }
}

fn assert_sections_carried_across_pages(chapter: &Chapter) {
    let heading_texts = |piece: &ChapterPiece| -> Vec<String> {
        piece
            .section
            .iter()
            .map(|heading| heading.text.clone())
            .collect()
    };
    assert!(
        on_page(chapter, 3)
            .take_while(|piece| !matches!(piece.detail, PieceDetail::Heading { .. }))
            .any(|piece| matches!(piece.detail, PieceDetail::Table { .. })
                && heading_texts(piece).contains(&"Forward Pricing".to_owned())),
        "a table above page 3's first heading is not under Forward Pricing"
    );
    assert!(
        on_page(chapter, 3).any(|piece| {
            let under = heading_texts(piece);
            !matches!(piece.detail, PieceDetail::Heading { .. })
                && under.contains(&"The Delta".to_owned())
                && under.contains(&"Rate of Change".to_owned())
        }),
        "nothing sits under both The Delta and Rate of Change"
    );
}

fn assert_math_page(chapter: &Chapter) {
    for label in ["(7.3)", "(7.4)"] {
        let formula = find_labelled(chapter, 4, label);
        let PieceDetail::Formula { statement, .. } = &formula.detail else {
            panic!("{label} is not a formula");
        };
        assert!(!statement.trim().is_empty());
        // Both formulas are printed over two lines and the second opens with a multiplication
        // sign. The model must not swap what is printed for the textbook form of the indices.
        let written = formula.content.replace(' ', "");
        assert!(
            written.contains("\\times"),
            "{label} lost its multiplication sign"
        );
        if label == "(7.4)" {
            assert!(
                written.contains("C_{i|1:j-1}(u_{k,i}|"),
                "(7.4) does not read as printed: {}",
                formula.content
            );
        }
        assert!(
            formula
                .file
                .extension()
                .is_some_and(|extension| extension == "tex")
        );
        assert!(
            formula
                .relationships
                .iter()
                .any(|link| link.kind == RelationshipKind::Introduces
                    && link.to == formula.id
                    && is_text(chapter, link.from)),
            "{label} has no text piece introducing it"
        );
    }
    assert!(
        on_page(chapter, 4).any(|piece| cites(piece).contains(&(CiteKind::Equation, "(4.18)"))),
        "no text piece on page 4 cites (4.18)"
    );
}

fn assert_chart_page(chapter: &Chapter) {
    let chart = find_labelled(chapter, 5, "Figure 13-4");
    assert!(
        chart
            .relationships
            .iter()
            .any(|link| link.kind == RelationshipKind::Discusses
                && link.to == chart.id
                && is_text(chapter, link.from)),
        "no paragraph on page 5 discusses Figure 13-4"
    );
    for printed_word in [
        "Underlying price",
        "Theoretical profit or loss",
        "Spread 2 (ratio spread)",
        "48.40",
    ] {
        assert!(
            chart.content.contains(printed_word),
            "the chart lacks {printed_word:?}"
        );
    }
}

fn assert_tables_page(chapter: &Chapter) {
    let tables: Vec<(usize, usize)> = on_page(chapter, 3)
        .filter(|piece| matches!(piece.detail, PieceDetail::Table { .. }))
        .map(table_shape)
        .collect();
    assert!(tables.len() >= 2, "page 3 has {} tables", tables.len());
    let wide = find_labelled(chapter, 3, "Figure 7-2");
    assert_eq!(table_shape(wide), (5, 8));
    assert!(wide.content.contains("not applicable"));
    assert_eq!(table_shape(find_labelled(chapter, 3, "Figure 7-3")), (3, 2));
}

fn assert_footnote_page(chapter: &Chapter) {
    assert!(
        on_page(chapter, 2).any(|footnote| {
            matches!(&footnote.detail, PieceDetail::Footnote { marker: Some(marker), .. } if marker == "1")
                && footnote.relationships.iter().any(|link| {
                    link.kind == RelationshipKind::FootnoteOf
                        && link.from == footnote.id
                        && chapter.piece(link.to).is_some_and(|carrier| carrier.content.contains("[^1]"))
                        && is_text(chapter, link.to)
                })
        }),
        "the footnote with marker 1 has no footnote-of edge to a text piece holding [^1]"
    );
}

fn assert_volatility_surface_page(chapter: &Chapter) {
    let surface = find_labelled(chapter, 7, "Figure 24-12");
    assert!(
        on_page(chapter, 7).any(|piece| {
            let cited = cites(piece);
            cited.contains(&(CiteKind::Figure, "Figure 24-12"))
                && cited.contains(&(CiteKind::Figure, "Figure 24-13"))
                && piece.relationships.iter().any(|link| {
                    link.kind == RelationshipKind::Discusses
                        && link.from == piece.id
                        && link.to == surface.id
                })
        }),
        "no text piece cites both figures and discusses Figure 24-12"
    );
}

fn assert_mixed_page(chapter: &Chapter) {
    assert!(
        on_page(chapter, 6).any(|piece| {
            matches!(piece.detail, PieceDetail::Table { .. })
                && piece
                    .relationships
                    .iter()
                    .any(|link| link.kind == RelationshipKind::Discusses && link.to == piece.id)
        }),
        "no paragraph on page 6 discusses its table"
    );
}

/// What the committed page 1 shows about the plain text page: Haiku copied it, no Sonnet call
/// was made, and the pale chapter number beside the title was read.
///
/// A live run is not held to this. The tagger sometimes takes the shaded corner block of that
/// page for a picture, which rightly sends the page to Sonnet, and Haiku sometimes misses the
/// number.
fn assert_text_page_was_copied_by_haiku(chapter_folder: &Path) {
    let chapter = read_chapter(chapter_folder).unwrap();
    let opening_heading = on_page(&chapter, 1)
        .find(|piece| matches!(piece.detail, PieceDetail::Heading { .. }))
        .unwrap();
    assert!(
        matches!(&opening_heading.detail, PieceDetail::Heading { printed_number: Some(number), .. } if number == "1"),
        "the chapter number printed beside the opening title was missed"
    );
    let first_page = std::fs::read_to_string(chapter_folder.join("page-num-1/page.json")).unwrap();
    let first_page: serde_json::Value = serde_json::from_str(&first_page).unwrap();
    assert_eq!(first_page["conversion"]["route"], "haiku-copy");
    for call in first_page["conversion"]["calls"].as_array().unwrap() {
        assert_ne!(call["step"], "transcribe");
    }
}

#[test]
fn sample_chapter_reads_back_in_order_with_sections_and_relationships() {
    assert_sample_chapter(&committed_chapter());
    assert_text_page_was_copied_by_haiku(&committed_chapter());
}
