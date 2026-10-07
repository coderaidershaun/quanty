//! Answer text with its citations and tables. For now the Markdown source is shown as it is,
//! followed by one chip for each citation.

use eframe::egui;

use super::Media;
use crate::theme::TextRole;
use crate::widgets::CitationChip;

/// A block of Markdown with the numbers of the results it cites.
#[derive(Debug, Clone, Copy)]
pub struct RichText<'a> {
    pub markdown: &'a str,
    pub cites: &'a [usize],
    pub role: TextRole,
    /// The citation to draw as chosen.
    pub selected: Option<usize>,
}

impl<'a> RichText<'a> {
    pub fn new(markdown: &'a str, role: TextRole) -> Self {
        RichText {
            markdown,
            cites: &[],
            role,
            selected: None,
        }
    }

    pub fn cites(mut self, cites: &'a [usize]) -> Self {
        self.cites = cites;
        self
    }

    pub fn selected(mut self, cite: Option<usize>) -> Self {
        self.selected = cite;
        self
    }
}

/// What a click on the text asked for. Every copy in the app goes through
/// `Intent::CopyText`, so the text never reaches the clipboard from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clicked {
    Citation(usize),
    /// "Copy text" or "Copy table" was chosen. It holds the stored source.
    CopyText(String),
}

/// Draws the text, then a chip for each citation. Returns the click, if there was one.
#[must_use = "a click or a copy request is lost"]
pub fn show(ui: &mut egui::Ui, _media: &mut Media, text: &RichText<'_>) -> Option<Clicked> {
    let mut clicked = None;
    ui.add(egui::Label::new(text.role.rich(text.markdown)).wrap());
    ui.horizontal_wrapped(|ui| {
        for &number in text.cites {
            let chip = CitationChip::new(number).selected(text.selected == Some(number));
            if ui.add(chip).clicked() {
                clicked = Some(Clicked::Citation(number));
            }
        }
    });
    clicked
}

/// Draws a Markdown table. It only ever returns `CopyText`, and the stand-in never does.
#[must_use = "a copy request is lost"]
pub fn table(
    ui: &mut egui::Ui,
    _media: &mut Media,
    markdown: &str,
    role: TextRole,
) -> Option<Clicked> {
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.label(egui::RichText::new(markdown).font(role.code_font()));
    });
    None
}

/// Counts what the text layouts did, for tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutStats {
    pub texts_parsed: u64,
    pub layouts_built: u64,
    pub layouts_held: usize,
}

/// The laid-out text that is kept between frames.
#[derive(Debug, Default)]
pub struct Layouts {
    stats: LayoutStats,
}

impl Layouts {
    pub fn new() -> Layouts {
        Layouts::default()
    }

    pub fn poll(&mut self, _ctx: &egui::Context) {}

    pub fn stats(&self) -> LayoutStats {
        self.stats
    }
}
