//! The one table of keyboard shortcuts: the shell reads it to bind the keys, and the help sheet
//! reads it to print them.

use std::fmt;

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
    R,
    S,
    Slash,
    Period,
    Escape,
    OpenBracket,
    CloseBracket,
}

impl KeyName {
    fn label(self) -> &'static str {
        match self {
            KeyName::Num1 => "1",
            KeyName::Num2 => "2",
            KeyName::Num3 => "3",
            KeyName::Num4 => "4",
            KeyName::J => "J",
            KeyName::K => "K",
            KeyName::R => "R",
            KeyName::S => "S",
            KeyName::Slash => "/",
            KeyName::Period => ".",
            KeyName::Escape => "Esc",
            KeyName::OpenBracket => "[",
            KeyName::CloseBracket => "]",
        }
    }
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

impl fmt::Display for Chord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.shift {
            formatter.write_str("⇧")?;
        }
        if self.command {
            formatter.write_str("⌘")?;
        }
        formatter.write_str(self.key.label())
    }
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
    pub help: std::borrow::Cow<'static, str>,
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
const fn row(
    chord: Chord,
    when: When,
    repeats: bool,
    intent: Intent,
    help: &'static str,
) -> Shortcut {
    Shortcut {
        chord,
        when,
        repeats,
        intent,
        help: std::borrow::Cow::Borrowed(help),
    }
}

/// Read from the top, so the most specific row comes first.
// A table reads best with one row on each line.
#[rustfmt::skip]
pub static SHORTCUTS: [Shortcut; 15] = [
    row(Chord { command: true, shift: true, key: KeyName::S }, When::Always, false, Intent::ShareAnswer,
        "Copy the answer with its citations"),
    row(command(KeyName::Num1), When::Always, false, Intent::OpenTab(Tab::Ask), "Go to Ask"),
    row(command(KeyName::Num2), When::Always, false, Intent::OpenTab(Tab::Library), "Go to Library"),
    row(command(KeyName::Num3), When::Always, false, Intent::OpenTab(Tab::Ingest), "Go to Ingest"),
    row(command(KeyName::Num4), When::Always, false, Intent::OpenTab(Tab::Datasets), "Go to Datasets"),
    row(command(KeyName::K), When::Always, false, Intent::FocusAskBar, "Write a question"),
    row(command(KeyName::Period), When::Always, false, Intent::CancelAsk, "Stop the search or the answer"),
    row(command(KeyName::CloseBracket), When::Always, true, Intent::TurnPage(1), "Next page of the source"),
    row(command(KeyName::OpenBracket), When::Always, true, Intent::TurnPage(-1), "Previous page of the source"),
    row(command(KeyName::R), When::Always, false, Intent::RecheckHealth, "Check the services again"),
    row(command(KeyName::Slash), When::Always, false, Intent::ToggleHelp, "Show or hide the shortcuts"),
    row(plain(KeyName::Slash), When::NotTyping, false, Intent::FocusAskBar, "Write a question"),
    row(plain(KeyName::Escape), When::NotTyping, false, Intent::StopOrRestore,
        "Stop the search or the answer, or put a maximised panel back"),
    row(plain(KeyName::J), When::NotTyping, true, Intent::StepResult(1), "Next result"),
    row(plain(KeyName::K), When::NotTyping, true, Intent::StepResult(-1), "Previous result"),
];

/// Keys that a panel handles itself, for the help sheet only: the key, and what it does.
pub static PANEL_KEYS: [(&str, &str); 3] = [
    ("Enter", "Ask the question in the box"),
    ("⌘+  ⌘−  ⌘0", "Zoom the page while the pointer is over it"),
    ("Esc", "Close a sheet"),
];
