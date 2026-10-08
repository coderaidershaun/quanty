//! The converter's rules for a figure's rectangle: which ones can be cut, and how much is added
//! around one. A rectangle that passed them has its own type, so nothing checks it again.

use crate::content::PageBox;

/// Added to every side of the rectangle that is cut, in thousandths of the page. It is under the
/// narrowest gap measured between a figure and the text next to it, and it forgives an edge a
/// little too tight.
const FIGURE_PADDING: i32 = 15;
/// A rectangle with a side shorter than this, in thousandths of the page, cannot be a figure. It
/// catches a reply written as fractions of the page.
pub(super) const MIN_FIGURE_SIDE: i32 = 20;

/// A rectangle that can be cut out of its page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UsableBox(PageBox);

impl UsableBox {
    /// Fails with the first reason the rectangle is unusable, checked in a fixed order.
    pub(super) fn new(area: PageBox) -> Result<Self, &'static str> {
        let sides = [area.left, area.top, area.right, area.bottom];
        if sides.iter().any(|side| !(0..=1000).contains(side)) {
            return Err("a side is not between 0 and 1000");
        }
        if area.left >= area.right {
            return Err("left is not less than right");
        }
        if area.top >= area.bottom {
            return Err("top is not less than bottom");
        }
        if area.right <= 100 && area.bottom <= 100 {
            return Err(
                "every number is 100 or less, which reads as percentages or fractions, not thousandths",
            );
        }
        if area.right - area.left < MIN_FIGURE_SIDE || area.bottom - area.top < MIN_FIGURE_SIDE {
            return Err("it is too small to be a figure");
        }
        Ok(Self(area))
    }

    pub(super) fn page_box(self) -> PageBox {
        self.0
    }

    /// Still usable, because padding only makes a rectangle bigger and keeps it inside the page.
    pub(super) fn padded(self) -> Self {
        Self(padded(self.0))
    }
}

pub(super) fn padded(area: PageBox) -> PageBox {
    PageBox {
        left: (area.left - FIGURE_PADDING).max(0),
        top: (area.top - FIGURE_PADDING).max(0),
        right: (area.right + FIGURE_PADDING).min(1000),
        bottom: (area.bottom + FIGURE_PADDING).min(1000),
    }
}
