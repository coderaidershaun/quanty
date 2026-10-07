//! Formulas as pictures: typeset off the window's thread, kept as textures, and drawn on the
//! text baseline. A formula that cannot be typeset shows its LaTeX source instead.

mod cache;
mod image;
mod show;
mod typeset;

pub use self::cache::Math;
pub use self::image::{Display, MathImage, MathRef, MathState};
pub use self::show::show;
