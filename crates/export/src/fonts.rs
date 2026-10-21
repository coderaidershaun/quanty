//! The font files a document is printed with. The desktop app draws its reading text with the
//! same files, so a printed page and the window read alike.

pub const LITERATA: &[u8] = include_bytes!("../assets/fonts/Literata[opsz,wght].ttf");
pub const LITERATA_ITALIC: &[u8] = include_bytes!("../assets/fonts/Literata-Italic[opsz,wght].ttf");
pub const SOURCE_CODE: &[u8] = include_bytes!("../assets/fonts/SourceCodePro[wght].ttf");
pub const STIX_TWO_MATH: &[u8] = include_bytes!("../assets/fonts/STIXTwoMath-Regular.otf");
