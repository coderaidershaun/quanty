//! Checks what a panel gets when it hands stored text to the media code: the text is read as the
//! pipeline wrote it, laid out once, and a click comes back as the source to copy.

mod table;
mod text;

use eframe::egui;
use eframe::egui::epaint::text::ByteRangeExt as _;
use egui_kittest::Harness;
use gui::contract::Intent;
use gui::media::rich_text::Clicked;
use gui::testkit::Host;

struct Painted {
    text: String,
    right: f32,
    baseline: f32,
    sections: Vec<(String, egui::text::TextFormat)>,
}

fn painted(harness: &Harness<'_, Host>) -> Vec<Painted> {
    fn collect(shape: &egui::Shape, found: &mut Vec<Painted>) {
        match shape {
            egui::Shape::Text(text) => {
                let galley = &text.galley;
                let first = galley.rows.first().and_then(|row| row.row.glyphs.first());
                found.push(Painted {
                    text: galley.text().to_owned(),
                    right: text.pos.x + galley.size().x,
                    baseline: text.pos.y + first.map_or(0.0, |glyph| glyph.pos.y),
                    sections: galley
                        .job
                        .sections
                        .iter()
                        .map(|section| {
                            let words = section.byte_range.slice(&galley.job.text);
                            (words.to_owned(), section.format.clone())
                        })
                        .collect(),
                });
            }
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        collect(&clipped.shape, &mut found);
    }
    found
}

/// All the painted text with no white space, so that a line break cannot split what is asked.
fn squeezed(painted: &[Painted]) -> String {
    painted
        .iter()
        .flat_map(|piece| piece.text.chars())
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// What a panel does with what `show` returns.
fn route(click: Option<Clicked>, intents: &mut Vec<Intent>) {
    match click {
        Some(Clicked::Citation(number)) => intents.push(Intent::SelectResult(number)),
        Some(Clicked::CopyText(source)) => intents.push(Intent::CopyText(source)),
        None => {}
    }
}

/// The five-column table of the sample chapter, as the page reader stored it.
const TABLE: &str = "| | If domestic rates rise | If domestic rates fall | If foreign rates rise | If foreign rates fall |
|---|---|---|---|---|
| stock option calls will | rise | fall | not applicable | not applicable |
| stock option puts will | fall | rise | not applicable | not applicable |
| futures option calls (stock-type settlement) | fall | fall | not applicable | not applicable |
| futures option puts (stock-type settlement) | fall | fall | not applicable | not applicable |
| futures option calls (futures-type settlement) | no effect | no effect | not applicable | not applicable |
| futures option puts (futures-type settlement) | no effect | no effect | not applicable | not applicable |
| foreign currency option calls | rise | fall | fall | rise |
| foreign currency option puts | fall | rise | rise | fall |";

const LAST_HEADER: &str = "If foreign rates fall";
