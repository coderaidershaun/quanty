//! Maps a chapter that the test writes by hand to its items, with no store and no model, for the
//! cases that no sample chapter has.

use std::fs;
use std::path::Path;

use ocr::read_chapter;
use rag_ingestion::chapter_items;
use serde_json::{Value, json};

fn write_json(file: &Path, value: &Value) {
    fs::write(file, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

#[test]
fn a_chunk_keeps_the_figure_table_and_equation_that_its_footnote_cites() {
    let folder = tempfile::tempdir().unwrap();
    let page = folder.path().join("page-num-1");
    fs::create_dir(&page).unwrap();
    write_json(
        &folder.path().join("chapter.json"),
        &json!({
            "format-version": 1,
            "book-title": "Notes on Footnotes",
            "chapter-number": 1,
            "chapter-name": "What a Footnote Cites",
            "source-file": "chapter-1-what-a-footnote-cites.pdf",
            "source-sha256": "f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0",
            "page-count": 1,
            "finished": true,
        }),
    );
    // The first footnote belongs to the text piece, so it is part of the chunk of that piece. The
    // second belongs to no piece, so it is a chunk by itself.
    write_json(
        &page.join("page.json"),
        &json!({
            "format-version": 1,
            "page-position": 1,
            "pieces": [
                {
                    "number": 1,
                    "file": "01-text.md",
                    "kind": "text",
                    "cites": [{ "kind": "footnote", "label": "1" }],
                },
                {
                    "number": 2,
                    "file": "02-footnote.md",
                    "kind": "footnote",
                    "marker": "1",
                    "cites": [
                        { "kind": "figure", "label": "Figure 2-1" },
                        { "kind": "footnote", "label": "2" },
                    ],
                },
                {
                    "number": 3,
                    "file": "03-footnote.md",
                    "kind": "footnote",
                    "marker": "2",
                    "cites": [
                        { "kind": "table", "label": "Table 2-3" },
                        { "kind": "equation", "label": "(2.4)" },
                    ],
                },
            ],
            "relationships": [{ "kind": "footnote-of", "from": 2, "to": 1 }],
        }),
    );
    for (file, text) in [
        (
            "01-text.md",
            "Implied volatility is not the same at every strike.[^1]",
        ),
        ("02-footnote.md", "Figure 2-1 draws it as a surface.[^2]"),
        (
            "03-footnote.md",
            "Table 2-3 lists the inputs of equation (2.4).",
        ),
    ] {
        fs::write(page.join(file), format!("{text}\n")).unwrap();
    }

    let items = chapter_items(&read_chapter(folder.path()).unwrap());

    let cites: Vec<Vec<String>> = items.into_iter().map(|item| item.payload.cites).collect();
    assert_eq!(
        cites,
        [vec!["Figure 2-1"], vec!["Table 2-3", "(2.4)"]],
        "each chunk keeps what its footnote cites, and never a footnote marker"
    );
}
