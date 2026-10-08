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

/// Read from the top, so the most specific row comes first.
pub static SHORTCUTS: [Shortcut; 14] = [
    Shortcut {
        chord: Chord {
            command: true,
            shift: true,
            key: KeyName::S,
        },
        when: When::Always,
        repeats: false,
        intent: Intent::ShareAnswer,
        help: "Copy the answer with its citations",
    },
    Shortcut {
        chord: command(KeyName::Num1),
        when: When::Always,
        repeats: false,
        intent: Intent::OpenTab(Tab::Ask),
        help: "Go to Ask",
    },
    Shortcut {
        chord: command(KeyName::Num2),
        when: When::Always,
        repeats: false,
        intent: Intent::OpenTab(Tab::Library),
        help: "Go to Library",
    },
    Shortcut {
        chord: command(KeyName::Num3),
        when: When::Always,
        repeats: false,
        intent: Intent::OpenTab(Tab::Ingest),
        help: "Go to Ingest",
    },
    Shortcut {
        chord: command(KeyName::K),
        when: When::Always,
        repeats: false,
        intent: Intent::FocusAskBar,
        help: "Write a question",
    },
    Shortcut {
        chord: command(KeyName::Period),
        when: When::Always,
        repeats: false,
        intent: Intent::CancelAsk,
        help: "Stop the search or the answer",
    },
    Shortcut {
        chord: command(KeyName::CloseBracket),
        when: When::Always,
        repeats: true,
        intent: Intent::TurnPage(1),
        help: "Next page of the source",
    },
    Shortcut {
        chord: command(KeyName::OpenBracket),
        when: When::Always,
        repeats: true,
        intent: Intent::TurnPage(-1),
        help: "Previous page of the source",
    },
    Shortcut {
        chord: command(KeyName::R),
        when: When::Always,
        repeats: false,
        intent: Intent::RecheckHealth,
        help: "Check the services again",
    },
    Shortcut {
        chord: command(KeyName::Slash),
        when: When::Always,
        repeats: false,
        intent: Intent::ToggleHelp,
        help: "Show or hide the shortcuts",
    },
    Shortcut {
        chord: plain(KeyName::Slash),
        when: When::NotTyping,
        repeats: false,
        intent: Intent::FocusAskBar,
        help: "Write a question",
    },
    Shortcut {
        chord: plain(KeyName::Escape),
        when: When::NotTyping,
        repeats: false,
        intent: Intent::CancelAsk,
        help: "Stop the search or the answer",
    },
    Shortcut {
        chord: plain(KeyName::J),
        when: When::NotTyping,
        repeats: true,
        intent: Intent::StepResult(1),
        help: "Next result",
    },
    Shortcut {
        chord: plain(KeyName::K),
        when: When::NotTyping,
        repeats: true,
        intent: Intent::StepResult(-1),
        help: "Previous result",
    },
];

/// Keys that a panel handles itself, for the help sheet only: the key, and what it does.
pub static PANEL_KEYS: [(&str, &str); 3] = [
    ("Enter", "Ask the question in the box"),
    ("⌘+  ⌘−  ⌘0", "Zoom the page while the pointer is over it"),
    ("Esc", "Close a sheet"),
];
