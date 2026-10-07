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
    Figures,
    Formulas,
    Tables,
    Concepts,
}

impl SourceTab {
    const ALL: [SourceTab; 5] = [
        SourceTab::Page,
        SourceTab::Figures,
        SourceTab::Formulas,
        SourceTab::Tables,
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
            SourceTab::Figures => "Figures",
            SourceTab::Formulas => "Formulas",
            SourceTab::Tables => "Tables",
            SourceTab::Concepts => "Concepts",
        }
    }
}

/// The pieces of the shown page that each tab lists, as places in `PageView::pieces`. It is
/// worked out again only when the page changes.
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

    /// The places of the pieces that `tab` lists.
    pub(super) fn of(&self, tab: SourceTab) -> &[usize] {
        match tab {
            SourceTab::Figures => &self.figures,
            SourceTab::Formulas => &self.formulas,
            SourceTab::Tables => &self.tables,
            SourceTab::Page | SourceTab::Concepts => &[],
        }
    }
}

/// What the body of a tab is given besides its own state.
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
    /// Names a list's scroll position. A new page, or a new document, starts at the top.
    pub(super) fn scroll_id(&self, tab: SourceTab) -> (SourceTab, u64, u32) {
        (tab, self.generation, self.page.page)
    }
}

/// The tab that can mark `piece`, the page picture first.
pub(super) fn tab_for(page: &PageView, piece: &PagePiece) -> SourceTab {
    if page.image.is_none() {
        return SourceTab::Page;
    }
    match piece.kind {
        PieceKind::Figure if piece.cut.is_none() => SourceTab::Figures,
        PieceKind::Formula => SourceTab::Formulas,
        PieceKind::Table => SourceTab::Tables,
        PieceKind::Figure | PieceKind::Heading { .. } | PieceKind::Text | PieceKind::Footnote => {
            SourceTab::Page
        }
    }
}

/// The five tabs with their counts. A count that is not known is `None`, not 0.
pub(super) fn tab_list(nav: &SourceNav, index: &PieceIndex) -> [Tab<'static>; 5] {
    let has_page = nav.page.ready().is_some();
    SourceTab::ALL.map(|tab| {
        let count = match tab {
            SourceTab::Page => None,
            SourceTab::Concepts => nav.concepts.ready().map(Vec::len),
            SourceTab::Figures | SourceTab::Formulas | SourceTab::Tables => {
                has_page.then(|| index.of(tab).len())
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Failure, FailureKind, ImageRef, Loadable, PageBox, PieceKind};
    use crate::panels::source::Local;
    use crate::state::{Shared, SourceTarget};
    use crate::testkit::sample;
    use crate::theme::Icon;

    const CUT: PageBox = PageBox {
        left: 10,
        top: 20,
        right: 900,
        bottom: 400,
    };

    fn piece(number: u32, kind: PieceKind, cut: Option<PageBox>) -> PagePiece {
        PagePiece {
            number,
            kind,
            label: None,
            name: None,
            caption: None,
            text: String::new(),
            image: None,
            cut,
        }
    }

    fn page(has_picture: bool) -> PageView {
        PageView {
            page: 3,
            page_count: 7,
            image: has_picture.then(|| ImageRef {
                path: "page.png".into(),
            }),
            pieces: vec![
                piece(1, PieceKind::Heading { rank: 2 }, None),
                piece(2, PieceKind::Text, None),
                piece(3, PieceKind::Figure, Some(CUT)),
                piece(4, PieceKind::Figure, None),
                piece(5, PieceKind::Formula, None),
                piece(6, PieceKind::Formula, None),
                piece(7, PieceKind::Table, None),
                piece(8, PieceKind::Footnote, None),
            ],
            ..PageView::default()
        }
    }

    fn shared_with(page: PageView, piece: Option<u32>) -> Shared {
        let mut shared = Shared::default();
        shared.source.generation = 1;
        shared.source.target = Some(SourceTarget {
            doc: page.doc,
            page: page.page,
            piece,
        });
        shared.source.page = Loadable::Ready(page);
        shared
    }

    fn counts(shared: &Shared, index: &PieceIndex) -> Vec<Option<usize>> {
        tab_list(&shared.source, index)
            .iter()
            .map(|tab| tab.count)
            .collect()
    }

    /// The tab and the pending reveal after a result that names `piece` is shown.
    fn opened(page: PageView, piece: Option<u32>) -> (SourceTab, Option<u32>) {
        let mut local = Local {
            generation: 1,
            tab: SourceTab::Tables,
            ..Local::default()
        };
        local.follow(&shared_with(page, piece));
        (local.tab, local.reveal)
    }

    #[test]
    fn pieces_are_counted_by_kind_and_a_target_opens_the_tab_that_can_mark_it() {
        let mut index = PieceIndex::default();
        let waiting = Shared::default();
        assert_eq!(
            counts(&waiting, &index),
            vec![None; 5],
            "no page: no count, and not 0"
        );

        let mut shared = shared_with(page(true), None);
        index.refresh(shared.source.page.ready().expect("a page is shown"));
        assert_eq!(
            counts(&shared, &index),
            vec![None, Some(2), Some(2), Some(1), None],
            "pieces by kind; the concepts are not read yet"
        );
        let concept = sample::page_concepts().remove(0);
        shared.source.concepts = Loadable::Ready(vec![concept; 3]);
        assert_eq!(counts(&shared, &index)[4], Some(3));
        shared.source.concepts = Loadable::Failed(Failure::new(FailureKind::Internal, "gone"));
        let failed = tab_list(&shared.source, &index);
        assert_eq!(failed[4].icon, Some(Icon::WARNING));
        assert_eq!(failed[4].count, None);

        // One case for each kind of piece a result can name, with and without a page picture.
        assert_eq!(
            opened(page(true), Some(3)),
            (SourceTab::Page, Some(3)),
            "a figure with a rectangle is framed on the page"
        );
        assert_eq!(
            opened(page(true), Some(4)),
            (SourceTab::Figures, Some(4)),
            "a figure with no rectangle shows as a card"
        );
        assert_eq!(opened(page(true), Some(5)).0, SourceTab::Formulas);
        assert_eq!(opened(page(true), Some(7)).0, SourceTab::Tables);
        for text in [1, 2, 8] {
            assert_eq!(
                opened(page(true), Some(text)),
                (SourceTab::Page, Some(text)),
                "text has no rectangle: the page opens at its top"
            );
        }
        for number in 1..=8 {
            assert_eq!(
                opened(page(false), Some(number)).0,
                SourceTab::Page,
                "a chapter with no pictures shows every piece in its text"
            );
        }
        assert_eq!(
            opened(page(true), None),
            (SourceTab::Tables, None),
            "no piece: the open tab stays"
        );
        assert_eq!(
            opened(page(true), Some(99)),
            (SourceTab::Tables, None),
            "a piece the page does not hold: the open tab stays"
        );
    }
}
