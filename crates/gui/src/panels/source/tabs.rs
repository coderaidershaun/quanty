//! The five tabs of the source panel: what each one counts, which one a result opens, and the
//! strip that shows them.

use eframe::egui;

use crate::contract::{DocId, Intent, Loadable, PagePiece, PageView, PieceKind};
use crate::media::Media;
use crate::state::SourceNav;
use crate::theme::{Icon, Tone};
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
}

impl SourceTab {
    const ALL: [SourceTab; 5] = [
        SourceTab::Page,
        SourceTab::Cards(CardTab::Figures),
        SourceTab::Cards(CardTab::Formulas),
        SourceTab::Cards(CardTab::Tables),
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
        for (place, piece) in page.pieces.iter().enumerate() {
            match piece.kind {
                PieceKind::Figure => self.figures.push(place),
                PieceKind::Formula => self.formulas.push(place),
                PieceKind::Table => self.tables.push(place),
                PieceKind::Heading { .. } | PieceKind::Text | PieceKind::Footnote => {}
            }
        }
    }

    pub(super) fn of(&self, tab: CardTab) -> &[usize] {
        match tab {
            CardTab::Figures => &self.figures,
            CardTab::Formulas => &self.formulas,
            CardTab::Tables => &self.tables,
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
        PieceKind::Figure | PieceKind::Heading { .. } | PieceKind::Text | PieceKind::Footnote => {
            SourceTab::Page
        }
    }
}

/// A count that is not known is `None`, not 0.
pub(super) fn tab_list(nav: &SourceNav, index: &PieceIndex) -> [Tab<'static>; 5] {
    let has_page = nav.page.ready().is_some();
    SourceTab::ALL.map(|tab| {
        let count = match tab {
            SourceTab::Page => None,
            SourceTab::Concepts => nav.concepts.ready().map(Vec::len),
            SourceTab::Cards(tab) => has_page.then(|| index.of(tab).len()),
        };
        let has_failed = tab == SourceTab::Concepts && matches!(nav.concepts, Loadable::Failed(_));
        Tab {
            label: tab.label(),
            count,
            icon: has_failed.then_some(Icon::WARNING),
        }
    })
}

/// Draws the strip and returns the tab the person chose, never the active one.
pub(super) fn strip(ui: &mut egui::Ui, tabs: &[Tab<'_>], active: SourceTab) -> Option<SourceTab> {
    let chosen = TabStrip::new(tabs, active.place())
        .tone(Tone::Magenta)
        .compact()
        .show(ui)?;
    SourceTab::ALL.get(chosen).copied()
}
