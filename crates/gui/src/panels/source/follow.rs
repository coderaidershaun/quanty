//! What the source panel does when the document, the page or the target changes: start again,
//! or open the tab that can show the piece a result names.

use super::Local;
use super::tabs::tab_for;
use crate::contract::{PagePiece, PageView};
use crate::state::{Shared, SourceNav};

pub(super) fn marked_piece<'a>(nav: &SourceNav, page: &'a PageView) -> Option<&'a PagePiece> {
    let target = nav.target?;
    if (target.doc, target.page) != (page.doc, page.page) {
        return None;
    }
    let number = target.piece?;
    page.pieces.iter().find(|piece| piece.number == number)
}

impl Local {
    /// It runs first in every frame.
    pub(super) fn follow(&mut self, shared: &Shared) {
        let nav = &shared.source;
        if nav.generation != self.generation {
            *self = Local {
                generation: nav.generation,
                header: std::mem::take(&mut self.header),
                ..Local::default()
            };
        }
        let Some(page) = nav.page.ready() else {
            self.followed = None;
            return;
        };
        self.index.refresh(page);
        let Some(target) = nav.target else {
            return;
        };
        let seen = (target, shared.cues.source_shows);
        if (target.doc, target.page) != (page.doc, page.page) || self.followed == Some(seen) {
            return;
        }
        self.followed = Some(seen);
        match marked_piece(nav, page) {
            Some(piece) => {
                self.tab = tab_for(page, piece);
                self.reveal = Some(piece.number);
            }
            None => self.reveal = None,
        }
    }
}
