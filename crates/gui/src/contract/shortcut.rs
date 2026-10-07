//! The one table of keyboard shortcuts: the shell reads it to bind the keys, and the help sheet
//! reads it to print them.

use std::fmt;

use super::message::{Intent, Tab};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyName {
    Num1,
    Num2,
    Num3,
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

/// When a shortcut is read. `NotTyping` rows are skipped while a text box has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum When {
    Always,
    NotTyping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    pub command: bool,
    pub shift: bool,
    pub key: KeyName,
}

/// What the help sheet prints: "⌘K", "⇧⌘S", "/", "Esc".
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Shortcut {
    pub chord: Chord,
    pub when: When,
    /// A held key counts again. Paging and stepping want that; everything else counts once.
    pub repeats: bool,
    pub intent: Intent,
    pub help: &'static str,
}

const fn row(
    command: bool,
    shift: bool,
    key: KeyName,
    when: When,
    repeats: bool,
    intent: Intent,
    help: &'static str,
) -> Shortcut {
    Shortcut {
        chord: Chord {
            command,
            shift,
            key,
        },
        when,
        repeats,
        intent,
        help,
    }
}

/// Read from the top, so the most specific row comes first.
pub static SHORTCUTS: [Shortcut; 14] = [
    row(
        true,
        true,
        KeyName::S,
        When::Always,
        false,
        Intent::ShareAnswer,
        "Copy the answer with its citations",
    ),
    row(
        true,
        false,
        KeyName::Num1,
        When::Always,
        false,
        Intent::OpenTab(Tab::Ask),
        "Go to Ask",
    ),
    row(
        true,
        false,
        KeyName::Num2,
        When::Always,
        false,
        Intent::OpenTab(Tab::Library),
        "Go to Library",
    ),
    row(
        true,
        false,
        KeyName::Num3,
        When::Always,
        false,
        Intent::OpenTab(Tab::Ingest),
        "Go to Ingest",
    ),
    row(
        true,
        false,
        KeyName::K,
        When::Always,
        false,
        Intent::FocusAskBar,
        "Write a question",
    ),
    row(
        true,
        false,
        KeyName::Period,
        When::Always,
        false,
        Intent::CancelAsk,
        "Stop the search or the answer",
    ),
    row(
        true,
        false,
        KeyName::CloseBracket,
        When::Always,
        true,
        Intent::TurnPage(1),
        "Next page of the source",
    ),
    row(
        true,
        false,
        KeyName::OpenBracket,
        When::Always,
        true,
        Intent::TurnPage(-1),
        "Previous page of the source",
    ),
    row(
        true,
        false,
        KeyName::R,
        When::Always,
        false,
        Intent::RecheckHealth,
        "Check the services again",
    ),
    row(
        true,
        false,
        KeyName::Slash,
        When::Always,
        false,
        Intent::ToggleHelp,
        "Show or hide the shortcuts",
    ),
    row(
        false,
        false,
        KeyName::Slash,
        When::NotTyping,
        false,
        Intent::FocusAskBar,
        "Write a question",
    ),
    row(
        false,
        false,
        KeyName::Escape,
        When::NotTyping,
        false,
        Intent::CancelAsk,
        "Stop the search or the answer",
    ),
    row(
        false,
        false,
        KeyName::J,
        When::NotTyping,
        true,
        Intent::StepResult(1),
        "Next result",
    ),
    row(
        false,
        false,
        KeyName::K,
        When::NotTyping,
        true,
        Intent::StepResult(-1),
        "Previous result",
    ),
];

/// Keys that a panel handles itself, for the help sheet only: the key, and what it does.
pub static PANEL_KEYS: [(&str, &str); 3] = [
    ("Enter", "Ask the question in the box"),
    ("⌘+  ⌘−  ⌘0", "Zoom the page while the pointer is over it"),
    ("Esc", "Close a sheet"),
];
