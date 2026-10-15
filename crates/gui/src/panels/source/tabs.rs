//! The six tabs of the source panel: what each one counts, which one a result opens, and the
//! strip that shows them.

use eframe::egui;

use crate::contract::{DocId, Intent, PagePiece, PageView, PieceKind};
use crate::media::Media;
use crate::state::SourceNav;
use crate::theme::{Icon, Tone, size, space};
use crate::widgets::{Tab, TabStrip};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum SourceTab {
    #[default]
    Page,
    Cards(CardTab),
    Concepts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum CardTab {
    Figures,
    Formulas,
    Tables,
    Code,
}

impl SourceTab {
    const ALL: [SourceTab; 6] = [
        SourceTab::Page,
        SourceTab::Cards(CardTab::Figures),
        SourceTab::Cards(CardTab::Formulas),
        SourceTab::Cards(CardTab::Tables),
        SourceTab::Cards(CardTab::Code),
        SourceTab::Concepts,
    ];

    fn place(self) -> usize {
        SourceTab::ALL
            .iter()
            .position(|tab| *tab == self)
            .unwrap_or_default()
    }

    const fn label(self) -> &'static str {
        match self {
            SourceTab::Page => "Page",
            SourceTab::Cards(CardTab::Figures) => "Figures",
            SourceTab::Cards(CardTab::Formulas) => "Formulas",
            SourceTab::Cards(CardTab::Tables) => "Tables",
            SourceTab::Cards(CardTab::Code) => "Code",
            SourceTab::Concepts => "Concepts",
        }
    }
}

/// The pieces of the shown page that each tab lists, as places in `PageView::pieces`.
#[derive(Debug, Default)]
pub(super) struct PieceIndex {
    key: Option<(DocId, u32, usize)>,
    figures: Vec<usize>,
    formulas: Vec<usize>,
    tables: Vec<usize>,
    code: Vec<usize>,
}

impl PieceIndex {
    pub(super) fn refresh(&mut self, page: &PageView) {
        let key = (page.doc, page.page, page.pieces.len());
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.figures.clear();
        self.formulas.clear();
        self.tables.clear();
        self.code.clear();
        for (place, piece) in page.pieces.iter().enumerate() {
            match piece.kind {
                PieceKind::Figure => self.figures.push(place),
                PieceKind::Formula => self.formulas.push(place),
                PieceKind::Table => self.tables.push(place),
                PieceKind::Code => self.code.push(place),
                PieceKind::Heading { .. } | PieceKind::Text | PieceKind::Footnote => {}
            }
        }
    }

    pub(super) fn of(&self, tab: CardTab) -> &[usize] {
        match tab {
            CardTab::Figures => &self.figures,
            CardTab::Formulas => &self.formulas,
            CardTab::Tables => &self.tables,
            CardTab::Code => &self.code,
        }
    }
}

pub(super) struct TabCx<'a> {
    pub page: &'a PageView,
    /// The piece the target names, when the target is this page.
    pub target_piece: Option<&'a PagePiece>,
    pub index: &'a PieceIndex,
    /// The piece still to bring into view. The tab body clears it when it is done.
    pub reveal: &'a mut Option<u32>,
    /// Another document gives another number, so its lists start at the top.
    pub generation: u64,
    pub media: &'a mut Media,
    pub intents: &'a mut Vec<Intent>,
}

impl TabCx<'_> {
    /// A new page, or a new document, starts at the top.
    pub(super) fn scroll_id(&self, tab: SourceTab) -> (SourceTab, u64, u32) {
        (tab, self.generation, self.page.page)
    }
}

pub(super) fn tab_for(page: &PageView, piece: &PagePiece) -> SourceTab {
    if page.image.is_none() {
        return SourceTab::Page;
    }
    match piece.kind {
        PieceKind::Figure if piece.cut.is_none() => SourceTab::Cards(CardTab::Figures),
        PieceKind::Formula => SourceTab::Cards(CardTab::Formulas),
        PieceKind::Table => SourceTab::Cards(CardTab::Tables),
        PieceKind::Code => SourceTab::Cards(CardTab::Code),
        PieceKind::Figure | PieceKind::Heading { .. } | PieceKind::Text | PieceKind::Footnote => {
            SourceTab::Page
        }
    }
}

/// A count that is not known is `None`, not 0.
pub(super) fn tab_list(nav: &SourceNav, index: &PieceIndex) -> [Tab<'static>; 6] {
    let has_page = nav.page.ready().is_some();
    SourceTab::ALL.map(|tab| {
        let count = match tab {
            SourceTab::Page => None,
            SourceTab::Concepts => nav.concepts.ready().map(Vec::len),
            SourceTab::Cards(tab) => has_page.then(|| index.of(tab).len()),
        };
        Tab {
            count,
            ..Tab::new(tab.label())
        }
    })
}

/// Draws the strip and returns the tab the person chose, never the active one. A warning about
/// the concepts stands just after the last tab, which is Concepts, and shows its text on hover.
pub(super) fn strip(
    ui: &mut egui::Ui,
    tabs: &[Tab<'_>],
    active: SourceTab,
    concepts_warning: Option<&str>,
) -> Option<SourceTab> {
    // SMELL: the warning is placed by the size of the whole strip, so it counts on Concepts being
    // the last tab and on the strip's row being only as wide as its tabs. If the strip ever fills
    // its row, the warning lands at the far right. The tab strip should let a tab carry a warning.
    let drawn = ui.scope(|ui| TabStrip::new(tabs, active.place()).compact().show(ui));
    if let Some(warning) = concepts_warning {
        warn_after(ui, drawn.response.rect, warning);
    }
    SourceTab::ALL.get(drawn.inner?).copied()
}

// SMELL: the warning is a painted glyph with no name of its own, so a screen reader does not
// announce it. A person who uses one learns that the concepts failed only on the Concepts tab.
fn warn_after(ui: &mut egui::Ui, strip: egui::Rect, warning: &str) {
    let glyph = egui::Rect::from_center_size(
        egui::pos2(
            strip.right() + space::XS + size::GLYPH / 2.0,
            strip.center().y,
        ),
        egui::Vec2::splat(size::GLYPH),
    );
    let ink = Tone::Danger.swatch().text;
    Icon::WARNING.paint(ui.painter(), glyph.center(), size::GLYPH, ink);
    ui.interact(glyph, ui.id().with("concepts failed"), egui::Sense::hover())
        .on_hover_text(warning);
}
