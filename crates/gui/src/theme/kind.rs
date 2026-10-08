//! What a chip, a card tag, a graph node or a legend entry stands for, and the one tone and icon
//! of each kind.

use super::{Icon, Tone};
use crate::contract::{ItemKind, NodeKind, PieceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Concept,
    RelatedConcept,
    Text,
    Formula,
    Figure,
    Table,
}

impl Kind {
    /// A formula, a figure and a table share one colour, and `icon` tells them apart.
    pub const fn tone(self) -> Tone {
        match self {
            Kind::Concept => Tone::Blue,
            Kind::RelatedConcept => Tone::Purple,
            Kind::Text => Tone::Neutral,
            Kind::Formula | Kind::Figure | Kind::Table => Tone::Magenta,
        }
    }

    /// A concept and a related concept share one icon, and `tone` tells them apart.
    pub const fn icon(self) -> Icon {
        match self {
            Kind::Concept | Kind::RelatedConcept => Icon::CONCEPT,
            Kind::Text => Icon::TEXT,
            Kind::Formula => Icon::FORMULA,
            Kind::Figure => Icon::FIGURE,
            Kind::Table => Icon::TABLE,
        }
    }
}

impl From<ItemKind> for Kind {
    fn from(kind: ItemKind) -> Kind {
        match kind {
            ItemKind::Chunk => Kind::Text,
            ItemKind::Formula => Kind::Formula,
            ItemKind::Figure => Kind::Figure,
            ItemKind::Table => Kind::Table,
        }
    }
}

impl From<NodeKind> for Kind {
    fn from(kind: NodeKind) -> Kind {
        match kind {
            NodeKind::Concept => Kind::Concept,
            NodeKind::Related => Kind::RelatedConcept,
            NodeKind::Formula => Kind::Formula,
            NodeKind::Figure => Kind::Figure,
            NodeKind::Table => Kind::Table,
        }
    }
}

impl From<PieceKind> for Kind {
    fn from(kind: PieceKind) -> Kind {
        match kind {
            PieceKind::Heading { .. } | PieceKind::Text | PieceKind::Footnote => Kind::Text,
            PieceKind::Formula => Kind::Formula,
            PieceKind::Figure => Kind::Figure,
            PieceKind::Table => Kind::Table,
        }
    }
}
