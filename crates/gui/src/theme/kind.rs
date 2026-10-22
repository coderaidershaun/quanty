//! What a chip, a card tag, a graph node or a legend entry stands for, and the one tone and icon
//! of each kind.

use super::{Icon, Tone};
use contract::{ItemKind, NodeKind, PieceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Concept,
    RelatedConcept,
    Text,
    Formula,
    Figure,
    Table,
    Code,
}

impl Kind {
    /// Each kind has a tone of its own, except that code shares Ink with text, and its icon tells
    /// it apart.
    pub const fn tone(self) -> Tone {
        match self {
            Kind::Concept => Tone::Indigo,
            Kind::RelatedConcept => Tone::Purple,
            Kind::Text => Tone::Ink,
            Kind::Formula => Tone::Verdigris,
            Kind::Figure => Tone::Ochre,
            Kind::Table => Tone::Madder,
            Kind::Code => Tone::Ink,
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
            Kind::Code => Icon::CODE,
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
            ItemKind::Code => Kind::Code,
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
            PieceKind::Code => Kind::Code,
        }
    }
}
