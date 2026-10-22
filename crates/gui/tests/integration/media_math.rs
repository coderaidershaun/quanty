//! Checks that every formula in the committed chapters is typeset.

use std::path::{Path, PathBuf};

use eframe::egui;
use export::{Format, InsightsSheet, SheetInsight};
use gui::media::Offload;
use gui::media::math::{Math, MathRef, MathState};
use gui::media::rich_text::{DollarBlock, dollar_blocks};
use gui::testkit;
use gui::theme::TextRole;

#[derive(Default)]
struct Formulas {
    blocks: Vec<String>,
    spans: Vec<String>,
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
            } else if name == "insights.json" {
                let folder = path.parent().expect("a file has a folder");
                formulas.add_insights(&read_insights(folder));
            }
        }
        formulas
    }

    /// The formulas of one saved insights file, as the Insights view reads it: every insight, the
    /// cheat sheet, and the takeaways as the lines of a list.
    fn add_insights(&mut self, saved: &ocr::ChapterInsights) {
        let takeaways: Vec<String> = saved
            .takeaways
            .iter()
            .map(|takeaway| format!("- {takeaway}"))
            .collect();
        let takeaways = takeaways.join("\n");
        let insights = saved.insights.iter().map(|insight| insight.text.as_str());
        let texts = insights.chain([saved.cheat_sheet.as_str(), takeaways.as_str()]);
        for markdown in texts {
            for block in dollar_blocks(markdown) {
                match block {
                    DollarBlock::Formula(latex) => self.blocks.push(latex),
                    DollarBlock::Heading(text)
                    | DollarBlock::Text(text)
                    | DollarBlock::Table(text) => self.spans.extend(inline_spans(&text)),
                }
            }
        }
    }

    fn asked(&self) -> Vec<MathRef<'_>> {
        let blocks = self.blocks.iter().map(|latex| MathRef::block(latex));
        let inline = self.spans.iter().chain(&self.symbols);
        let inline = inline.map(|latex| MathRef::inline(latex, TextRole::Body));
        blocks.chain(inline).collect()
    }
}

fn read_insights(folder: &Path) -> ocr::ChapterInsights {
    ocr::ChapterInsights::read(folder)
        .expect("a sample insights file reads")
        .expect("the sample insights file is there")
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

    assert_the_exported_file_finds_the_formulas_the_view_finds(&testkit::samples_folder());
}

/// The exported HTML of each sample insights file typesets as many formulas as the Insights view
/// finds in it, so that the view and the file take the same text as formulas.
fn assert_the_exported_file_finds_the_formulas_the_view_finds(samples: &Path) {
    let folders: Vec<PathBuf> = files_under(samples)
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "insights.json"))
        .filter_map(|path| path.parent().map(Path::to_owned))
        .collect();
    assert!(!folders.is_empty(), "no insights.json was found");

    let temporary = tempfile::tempdir().expect("a temporary folder");
    for folder in folders {
        let saved = read_insights(&folder);
        let mut found = Formulas::default();
        found.add_insights(&saved);

        let sheet = InsightsSheet {
            document_title: "Sample".to_owned(),
            model: saved.model.clone(),
            insights: saved
                .insights
                .iter()
                .map(|insight| SheetInsight {
                    text: insight.text.clone(),
                    pages: insight.pages.clone(),
                })
                .collect(),
            takeaways: saved.takeaways.clone(),
            cheat_sheet: saved.cheat_sheet.clone(),
        };
        let path = temporary.path().join("insights.html");
        export::write(&export::insights_document(&sheet), Format::Html, &path)
            .expect("the HTML is written");
        let html = std::fs::read_to_string(&path).expect("the HTML can be read");

        let in_view = found.blocks.len() + found.spans.len();
        assert!(in_view > 0, "{} holds no formula", folder.display());
        assert_eq!(
            html.matches("<img class=\"math\"").count(),
            in_view,
            "the view and the file disagree on the formulas of {}",
            folder.display()
        );
    }
}
