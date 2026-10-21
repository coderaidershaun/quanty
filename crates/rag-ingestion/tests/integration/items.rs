//! Maps a chapter that the test writes by hand to its items, with no store and no model, for the
//! cases that no sample chapter has.

use std::fs;
use std::path::Path;

use ocr::read_chapter;
use rag_core::{ItemKind, ItemPayload};
use rag_ingestion::chapter_items;
use serde_json::{Value, json};

fn write_json(file: &Path, value: &Value) {
    fs::write(file, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

/// A piece entry of `page.json`. The file is named the way the converter names it.
fn piece(number: u32, kind: &str, fields: Value) -> Value {
    let extension = if kind == "code" { "txt" } else { "md" };
    let mut entry = json!({
        "number": number,
        "file": format!("{number:02}-{kind}.{extension}"),
        "kind": kind,
    });
    entry
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    entry
}

/// A page whose printed number is its position, with each piece and the text of its file.
fn write_page(chapter: &Path, position: u32, pieces: &[(Value, &str)]) {
    let page = chapter.join(format!("page-num-{position}"));
    fs::create_dir(&page).unwrap();
    let entries: Vec<&Value> = pieces.iter().map(|(entry, _)| entry).collect();
    write_json(
        &page.join("page.json"),
        &json!({
            "format-version": 1,
            "page-position": position,
            "printed-page-number": position.to_string(),
            "pieces": entries,
        }),
    );
    for (entry, text) in pieces {
        let file = entry["file"].as_str().unwrap();
        fs::write(page.join(file), format!("{text}\n")).unwrap();
    }
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
            "media-title": "Notes on Footnotes",
            "name": { "chapter": { "number": 1, "name": "What a Footnote Cites" } },
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

#[test]
fn a_code_piece_becomes_one_code_item() {
    let code = "    def call_price(spot, strike):\n        return max(spot - strike, 0)\n\n    print(\"a\\tb\")";
    let folder = tempfile::tempdir().unwrap();
    let page = folder.path().join("page-num-1");
    fs::create_dir(&page).unwrap();
    write_json(
        &folder.path().join("chapter.json"),
        &json!({
            "format-version": 1,
            "media-title": "Notes on Listings",
            "name": { "chapter": { "number": 1, "name": "A Short Listing" } },
            "source-file": "chapter-1-a-short-listing.pdf",
            "source-sha256": "c0dec0dec0dec0dec0dec0dec0dec0dec0dec0dec0dec0dec0dec0dec0dec0de",
            "page-count": 1,
            "finished": true,
        }),
    );
    write_json(
        &page.join("page.json"),
        &json!({
            "format-version": 1,
            "page-position": 1,
            "pieces": [
                { "number": 1, "file": "01-text.md", "kind": "text" },
                {
                    "number": 2,
                    "file": "02-code.txt",
                    "kind": "code",
                    "label": "Listing 1.1",
                    "caption": "A call price",
                    "language": "python",
                    "summary": "Gives the payoff of a call option.",
                },
            ],
        }),
    );
    fs::write(
        page.join("01-text.md"),
        "The listing below gives a payoff.\n",
    )
    .unwrap();
    fs::write(page.join("02-code.txt"), format!("{code}\n")).unwrap();

    let items = chapter_items(&read_chapter(folder.path()).unwrap());

    let codes: Vec<_> = items
        .iter()
        .filter(|item| item.payload.kind == ItemKind::Code)
        .collect();
    assert_eq!(codes.len(), 1, "{items:#?}");
    let item = codes[0];
    assert_eq!(item.payload.text, code);
    assert_eq!(item.payload.label.as_deref(), Some("Listing 1.1"));
    assert_eq!(item.payload.language.as_deref(), Some("python"));
    assert_eq!(
        item.input.text,
        format!(
            "Listing 1.1 A call price\n\nLanguage: python\n\nGives the payoff of a call option.\n\n{code}"
        )
    );
    assert_eq!(
        items.len(),
        2,
        "the code is an item of its own and is not packed into the chunk"
    );
    assert!(!items[0].payload.text.contains("def "), "{items:#?}");
}

#[test]
fn a_code_listing_that_page_breaks_cut_is_one_item_from_its_first_page_to_its_last() {
    let first_part = "template <typename T>\nclass RingBuffer {\npublic:";
    let second_part = "    bool push(const T& value) noexcept;\n    bool pop(T& out) noexcept;";
    let third_part = "private:\n    alignas(64) std::atomic<size_t> head_{0};\n};";
    let caption_opens = "int main() {\n    RingBuffer<int> buffer;\n}";
    let language_opens = "def feed(buffer):\n    return buffer.pop()";
    let passage_before = "def drain(buffer):\n    while buffer:\n        buffer.pop()";
    let label_opens = "print(drain(feed))";
    let same_page = "print(\"done\")";
    let folder = tempfile::tempdir().unwrap();
    write_json(
        &folder.path().join("chapter.json"),
        &json!({
            "format-version": 1,
            "media-title": "Notes on Listings",
            "name": { "chapter": { "number": 2, "name": "A Long Listing" } },
            "source-file": "chapter-2-a-long-listing.pdf",
            "source-sha256": "1009100910091009100910091009100910091009100910091009100910091009",
            "page-count": 7,
            "finished": true,
        }),
    );
    let text = |number| piece(number, "text", json!({}));
    let code = |number, summary: &str, mut fields: Value| {
        fields["summary"] = json!(summary);
        piece(number, "code", fields)
    };
    let python = json!({ "language": "python" });
    write_page(
        folder.path(),
        1,
        &[
            (text(1), "The listing below is one ring buffer."),
            (
                code(2, "S1", json!({ "caption": "C++", "language": "c++" })),
                first_part,
            ),
        ],
    );
    // A footnote between two parts does not stop the listing.
    write_page(
        folder.path(),
        2,
        &[
            (code(1, "S2", json!({ "language": "c++" })), second_part),
            (piece(2, "footnote", json!({ "marker": "1" })), "Noexcept."),
        ],
    );
    // A part whose language is not known joins the listing.
    write_page(folder.path(), 3, &[(code(1, "S3", json!({})), third_part)]);
    // A caption opens a new listing.
    write_page(
        folder.path(),
        4,
        &[(
            code(1, "S4", json!({ "caption": "C++", "language": "c++" })),
            caption_opens,
        )],
    );
    // Another known language opens a new listing.
    write_page(
        folder.path(),
        5,
        &[(code(1, "S5", python.clone()), language_opens)],
    );
    // A piece other than a footnote between two code pieces opens a new listing.
    write_page(
        folder.path(),
        6,
        &[
            (text(1), "Now the buffer is drained."),
            (code(2, "S6", python.clone()), passage_before),
        ],
    );
    // A label opens a new listing. A code piece on the same page is not a cut, so it opens one too.
    write_page(
        folder.path(),
        7,
        &[
            (
                code(
                    1,
                    "S7",
                    json!({ "label": "Listing 2", "language": "python" }),
                ),
                label_opens,
            ),
            (code(2, "S8", python), same_page),
        ],
    );

    let items = chapter_items(&read_chapter(folder.path()).unwrap());

    let codes: Vec<_> = items
        .iter()
        .filter(|item| item.payload.kind == ItemKind::Code)
        .collect();
    let texts: Vec<&str> = codes
        .iter()
        .map(|item| item.payload.text.as_str())
        .collect();
    let joined = format!("{first_part}\n{second_part}\n{third_part}");
    assert_eq!(
        texts,
        [
            joined.as_str(),
            caption_opens,
            language_opens,
            passage_before,
            label_opens,
            same_page
        ],
        "{items:#?}"
    );

    let listing = codes[0];
    assert_eq!(listing.payload.page, 1);
    assert_eq!(listing.payload.printed_page.as_deref(), Some("1"));
    assert_eq!(listing.payload.last_page, Some(3));
    assert_eq!(listing.payload.last_printed_page.as_deref(), Some("3"));
    assert_eq!(listing.payload.language.as_deref(), Some("c++"));
    assert_eq!(listing.payload.label, None);
    assert_eq!(
        listing.input.text,
        format!("C++\n\nLanguage: c++\n\nS1\n\nS2\n\nS3\n\n{joined}")
    );
    for alone in &codes[1..] {
        assert_eq!(alone.payload.last_page, None, "{alone:#?}");
        assert_eq!(alone.payload.last_printed_page, None, "{alone:#?}");
    }

    // The JSON of an item on one page is what a point stored before the fields held.
    let stored = serde_json::to_value(&codes[1].payload).unwrap();
    assert!(stored.get("last_page").is_none(), "{stored}");
    assert!(stored.get("last_printed_page").is_none(), "{stored}");
    assert_eq!(
        serde_json::from_value::<ItemPayload>(stored).unwrap(),
        codes[1].payload
    );
    let spanning = serde_json::to_value(&listing.payload).unwrap();
    assert_eq!(spanning["last_page"], json!(3));
    assert_eq!(spanning["last_printed_page"], json!("3"));
}
