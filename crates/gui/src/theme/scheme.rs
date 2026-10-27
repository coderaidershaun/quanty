//! Which of the two palettes the window draws with, and the word that keeps the choice between
//! visits.

use eframe::egui;

use super::Palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Scheme {
    /// The scheme the app opens in.
    #[default]
    Dark,
    Light,
}

impl Scheme {
    pub const ALL: [Scheme; 2] = [Scheme::Dark, Scheme::Light];

    /// Reads back what `as_stored` wrote. A missing value, or any other word, gives the scheme the
    /// app opens in, so a stored choice that was damaged can never leave the window unreadable.
    pub fn from_stored(value: Option<&str>) -> Scheme {
        match value {
            Some("light") => Scheme::Light,
            _ => Scheme::Dark,
        }
    }

    /// The word that is kept between visits.
    pub const fn as_stored(self) -> &'static str {
        match self {
            Scheme::Dark => "dark",
            Scheme::Light => "light",
        }
    }

    pub const fn other(self) -> Scheme {
        match self {
            Scheme::Dark => Scheme::Light,
            Scheme::Light => Scheme::Dark,
        }
    }

    pub const fn palette(self) -> &'static Palette {
        match self {
            Scheme::Dark => &Palette::DARK,
            Scheme::Light => &Palette::LIGHT,
        }
    }
}

impl From<Scheme> for egui::Theme {
    fn from(scheme: Scheme) -> egui::Theme {
        match scheme {
            Scheme::Dark => egui::Theme::Dark,
            Scheme::Light => egui::Theme::Light,
        }
    }
}

impl From<egui::Theme> for Scheme {
    fn from(theme: egui::Theme) -> Scheme {
        match theme {
            egui::Theme::Dark => Scheme::Dark,
            egui::Theme::Light => Scheme::Light,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_choice_is_dark_unless_it_says_light() {
        assert_eq!(Scheme::from_stored(None), Scheme::Dark);
        assert_eq!(Scheme::from_stored(Some("dark")), Scheme::Dark);
        assert_eq!(Scheme::from_stored(Some("light")), Scheme::Light);
        for garbage in ["Light", "", "blue"] {
            assert_eq!(
                Scheme::from_stored(Some(garbage)),
                Scheme::Dark,
                "{garbage:?}"
            );
        }
        for scheme in Scheme::ALL {
            assert_eq!(Scheme::from_stored(Some(scheme.as_stored())), scheme);
        }
    }
}
