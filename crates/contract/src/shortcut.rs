//! The one table of keyboard shortcuts, which the shell reads to bind the keys.

use super::message::{Intent, Tab};

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum KeyName {
    Num1,
    Num2,
    Num3,
    Num4,
    J,
    K,
    S,
    Slash,
    Period,
    Escape,
    OpenBracket,
    CloseBracket,
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum When {
    Always,
    NotTyping,
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct Chord {
    pub command: bool,
    pub shift: bool,
    pub key: KeyName,
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct Shortcut {
    pub chord: Chord,
    pub when: When,
    /// A held key counts again. Paging and stepping want that; everything else counts once.
    pub repeats: bool,
    pub intent: Intent,
}

const fn command(key: KeyName) -> Chord {
    Chord {
        command: true,
        shift: false,
        key,
    }
}

const fn plain(key: KeyName) -> Chord {
    Chord {
        command: false,
        shift: false,
        key,
    }
}

/// Lets the table below give each shortcut on one line.
const fn row(chord: Chord, when: When, repeats: bool, intent: Intent) -> Shortcut {
    Shortcut {
        chord,
        when,
        repeats,
        intent,
    }
}

/// Read from the top, so the most specific row comes first.
// A table reads best with one row on each line.
#[rustfmt::skip]
pub static SHORTCUTS: [Shortcut; 13] = [
    row(Chord { command: true, shift: true, key: KeyName::S }, When::Always, false, Intent::ShareAnswer),
    row(command(KeyName::Num1), When::Always, false, Intent::OpenTab(Tab::Ask)),
    row(command(KeyName::Num2), When::Always, false, Intent::OpenTab(Tab::Library)),
    row(command(KeyName::Num3), When::Always, false, Intent::OpenTab(Tab::Ingest)),
    row(command(KeyName::Num4), When::Always, false, Intent::OpenTab(Tab::Datasets)),
    row(command(KeyName::K), When::Always, false, Intent::FocusAskBar),
    row(command(KeyName::Period), When::Always, false, Intent::CancelAsk),
    row(command(KeyName::CloseBracket), When::Always, true, Intent::TurnPage(1)),
    row(command(KeyName::OpenBracket), When::Always, true, Intent::TurnPage(-1)),
    row(plain(KeyName::Slash), When::NotTyping, false, Intent::FocusAskBar),
    row(plain(KeyName::Escape), When::NotTyping, false, Intent::StopOrRestore),
    row(plain(KeyName::J), When::NotTyping, true, Intent::StepResult(1)),
    row(plain(KeyName::K), When::NotTyping, true, Intent::StepResult(-1)),
];
