//! Checks that every formula in the committed chapters is typeset.

use std::path::{Path, PathBuf};

use eframe::egui;
use gui::media::Offload;
use gui::media::math::{Math, MathRef, MathState};
use gui::testkit;
use gui::theme::TextRole;

/// Every formula the committed chapters hold, by the way the book writes it.
#[derive(Default)]
struct Formulas {
    /// The `.tex` files: whole formulas on a line of their own.
    blocks: Vec<String>,
    /// The `\( .. \)` spans inside the text files.
    spans: Vec<String>,
    /// The symbols a page lists in its `page.json`.
    symbols: Vec<String>,
}

impl Formulas {
    fn read(folder: &Path) -> Formulas {
        let mut formulas = Formulas::default();
        for path in files_under(folder) {
            let name = path.file_name().map(|name| name.to_string_lossy());
            let name = name.as_deref().unwrap_or_default();
            let read = || std::fs::read_to_string(&path).expect("a sample file can be read");
            if name.ends_with(".tex") {
                formulas.blocks.push(read().trim().to_owned());
            } else if name.ends_with(".md") {
                formulas.spans.extend(inline_spans(&read()));
            } else if name == "page.json" {
                let page: serde_json::Value =
                    serde_json::from_str(&read()).expect("a page.json is JSON");
                symbols_in(&page, &mut formulas.symbols);
            }
        }
        formulas
    }

    fn asked(&self) -> Vec<MathRef<'_>> {
        let blocks = self.blocks.iter().map(|latex| MathRef::block(latex));
        let inline = self.spans.iter().chain(&self.symbols);
        let inline = inline.map(|latex| MathRef::inline(latex, TextRole::Body));
        blocks.chain(inline).collect()
    }
}

fn files_under(folder: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let entries = std::fs::read_dir(folder).expect("a sample folder can be listed");
    for entry in entries {
        let path = entry.expect("a sample folder entry can be read").path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// The text inside each `\( .. \)` of `text`.
fn inline_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut open: Option<usize> = None;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' {
            match bytes[i + 1] {
                b'(' if open.is_none() => {
                    open = Some(i + 2);
                    i += 2;
                    continue;
                }
                b')' => {
                    if let Some(start) = open.take() {
                        spans.push(text[start..i].trim().to_owned());
                    }
                    i += 2;
                    continue;
                }
                _ => {
                    i += 2;
                    continue;
                }
            }
        }
        i += 1;
    }
    spans
}

fn symbols_in(value: &serde_json::Value, found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                match value.as_str() {
                    Some(symbol) if key == "symbol" => found.push(symbol.to_owned()),
                    _ => symbols_in(value, found),
                }
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| symbols_in(item, found)),
        _ => {}
    }
}

#[test]
fn every_formula_in_the_committed_samples_is_typeset() {
    let formulas = Formulas::read(&testkit::samples_folder());
    assert!(!formulas.blocks.is_empty(), "no .tex file was found");
    assert!(!formulas.spans.is_empty(), "no inline span was found");
    assert!(!formulas.symbols.is_empty(), "no symbol was found");

    let ctx = egui::Context::default();
    let mut math = Math::new(&ctx, Offload::Manual);
    let asked = formulas.asked();
    for formula in &asked {
        math.get(formula);
    }
    math.run_pending();
    math.poll(&ctx);

    let not_typeset: Vec<String> = asked
        .iter()
        .filter_map(|formula| match math.get(formula) {
            MathState::Ready(image) if image.width > 0.0 => None,
            other => Some(format!("{other:?}: {}", formula.latex)),
        })
        .collect();
    assert!(
        not_typeset.is_empty(),
        "{} of {} formulas were not typeset:\n{}",
        not_typeset.len(),
        asked.len(),
        not_typeset.join("\n")
    );
}
