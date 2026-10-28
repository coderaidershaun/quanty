//! One rule for reading the text of a tool's arguments, so that every tool treats blank text and
//! blank tags the same way, and one rule for how much text a read gives back and where the next
//! read goes on.

use std::fmt::Display;
use std::path::Path;
use std::str::FromStr;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rag_core::{EmptyTag, Tag};
use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// The text with no space at either end, or `None` when nothing is left.
pub(crate) fn non_blank(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// The trimmed text read as a `T`, or `None` when the text is blank.
pub(crate) fn parse_optional<T: FromStr>(text: Option<String>) -> Result<Option<T>, T::Err> {
    non_blank(text).map(|text| text.parse()).transpose()
}

/// The last part of the path as text, or `None` when the path has none.
pub(crate) fn file_name_of(path: impl AsRef<Path>) -> Option<String> {
    path.as_ref()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// Every tag of the list, or the error for the first one that is blank.
pub(crate) fn parse_tags<C: FromIterator<Tag>>(texts: Option<Vec<String>>) -> Result<C, EmptyTag> {
    texts
        .unwrap_or_default()
        .iter()
        .map(|tag| tag.parse::<Tag>())
        .collect()
}

/// The characters a read gives back unless the caller asks for another number.
pub(crate) const DEFAULT_MAX_CHARS: u32 = 24_000;

/// The most characters one read may give back: a client keeps a result of 2 MiB at most.
pub(crate) const LARGEST_MAX_CHARS: u32 = 200_000;

const CURSOR_VERSION: &str = "v1";

/// How many characters of a fingerprint a scope keeps.
const FINGERPRINT_CHARS: usize = 12;

#[derive(thiserror::Error, Debug)]
pub(crate) enum BudgetError {
    #[error("`max_chars` is 0; give a whole number from 1 to {LARGEST_MAX_CHARS}, or leave it out")]
    ZeroChars,

    #[error(
        "`max_chars` is {given}, more than a read gives, which is {LARGEST_MAX_CHARS}; give a smaller number and go on with `cursor`"
    )]
    TooManyChars { given: u32 },

    #[error("`cursor` is not from this read; call again without `cursor`")]
    ForeignCursor,
}

/// What a read reads: the tool and the facts that decide the text it gives. A cursor belongs to
/// the scope it was made in, so a cursor of another read is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Scope(String);

impl Scope {
    pub(crate) fn of(tool: &str) -> Scope {
        Scope(tool.to_owned())
    }

    pub(crate) fn with(mut self, part: impl Display) -> Scope {
        self.0.push(':');
        self.0.push_str(&part.to_string());
        self
    }

    /// For text that is long or free, such as a question: only a short fingerprint of it is kept.
    pub(crate) fn with_fingerprint(self, text: &str) -> Scope {
        let digest: String = Sha256::digest(text.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.with(&digest[..FINGERPRINT_CHARS])
    }

    pub(crate) fn cursor(&self, at: Position) -> String {
        let text = format!("{CURSOR_VERSION}.{}.{}.{}", at.unit, at.offset, self.0);
        URL_SAFE_NO_PAD.encode(text)
    }

    /// # Errors
    /// [`BudgetError::ForeignCursor`] when the text is not a cursor, or is one of another read.
    pub(crate) fn resume(&self, cursor: &str) -> Result<Position, BudgetError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(cursor.trim())
            .map_err(|_| BudgetError::ForeignCursor)?;
        let text = String::from_utf8(bytes).map_err(|_| BudgetError::ForeignCursor)?;
        let mut parts = text.splitn(4, '.');
        let (Some(CURSOR_VERSION), Some(unit), Some(offset), Some(scope)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(BudgetError::ForeignCursor);
        };
        let (Ok(unit), Ok(offset)) = (unit.parse(), offset.parse()) else {
            return Err(BudgetError::ForeignCursor);
        };
        if scope != self.0 {
            return Err(BudgetError::ForeignCursor);
        }
        Ok(Position { unit, offset })
    }
}

/// A place in a sequence of units: the unit, and the character of it to go on from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Position {
    pub(crate) unit: usize,
    pub(crate) offset: usize,
}

/// Whether a result stops short of the text, and how to read the rest.
#[derive(Serialize, JsonSchema, Debug, Default)]
pub(crate) struct Truncation {
    /// True when the text goes on past this result. Give `cursor` back to read the rest.
    truncated: bool,
    /// Give it back with the same arguments to go on. Only when `truncated`.
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
    /// The characters of text this result holds.
    #[serde(skip)]
    used: usize,
}

impl Truncation {
    pub(crate) fn is_truncated(&self) -> bool {
        self.truncated
    }

    pub(crate) fn used(&self) -> usize {
        self.used
    }

    /// A result that stops short of the text. The caller gives `cursor` back to read the rest.
    pub(crate) fn stopping_at(cursor: Option<String>, used: usize) -> Truncation {
        Truncation {
            truncated: true,
            cursor,
            used,
        }
    }

    /// The same result with this cursor in place of its own. A result that pages by whole rows
    /// never goes on inside a text that was cut, so it gives the cursor of its next row.
    pub(crate) fn paged_by(self, cursor: Option<String>) -> Truncation {
        Truncation { cursor, ..self }
    }

    pub(crate) fn whole(used: usize) -> Truncation {
        Truncation {
            used,
            ..Truncation::default()
        }
    }
}

/// The part of one unit that a read took.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Slice {
    pub(crate) unit: usize,
    /// The character of the unit that `text` starts at.
    pub(crate) offset: usize,
    pub(crate) text: String,
}

/// What a read took of its units, and whether it left some.
pub(crate) struct Taken {
    pub(crate) slices: Vec<Slice>,
    pub(crate) truncation: Truncation,
}

/// How much text a read may give, and where it starts. The text of a result is a sequence of
/// units; units are taken in order while they fit, and the one that does not fit is cut by
/// characters, never inside a character. The cursor goes on inside that unit, so the reads of one
/// text put together are the whole text, with no character lost or repeated.
pub(crate) struct Budget {
    scope: Scope,
    chars: usize,
    from: Position,
}

impl Budget {
    /// # Errors
    /// A `max_chars` of 0 or over [`LARGEST_MAX_CHARS`], and a `cursor` that is not from `scope`.
    pub(crate) fn new(
        scope: Scope,
        max_chars: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<Budget, BudgetError> {
        let chars = checked_max_chars(max_chars)?;
        let from = match non_blank(cursor.map(str::to_owned)) {
            Some(cursor) => scope.resume(&cursor)?,
            None => Position::default(),
        };
        Ok(Budget { scope, chars, from })
    }

    /// Takes the units from where the cursor says, as many characters as the budget allows.
    ///
    /// # Errors
    /// [`BudgetError::ForeignCursor`] when the cursor points past the units, which happens when
    /// the text it was made for has changed.
    pub(crate) fn take<S: AsRef<str>>(&self, units: &[S]) -> Result<Taken, BudgetError> {
        self.take_keeping_heads(units, |_| 0)
    }

    /// Like [`Budget::take`] for a result that is one text: the part of it that this read gives.
    ///
    /// # Errors
    /// As [`Budget::take`].
    pub(crate) fn take_text(&self, text: &str) -> Result<(String, Truncation), BudgetError> {
        let taken = self.take(&[text])?;
        let part = taken.slices.into_iter().map(|slice| slice.text).collect();
        Ok((part, taken.truncation))
    }

    /// Like [`Budget::take`], but the first `head(unit)` characters of a unit are never cut: a
    /// unit that would be cut inside its head is left whole for the cursor. Only the first unit of
    /// a read is cut there, so that every read gives something.
    ///
    /// # Errors
    /// As [`Budget::take`].
    pub(crate) fn take_keeping_heads<S: AsRef<str>>(
        &self,
        units: &[S],
        head: impl Fn(usize) -> usize,
    ) -> Result<Taken, BudgetError> {
        let Position {
            mut unit,
            mut offset,
        } = self.from;
        let starts_past_end = unit > units.len()
            || units
                .get(unit)
                .is_some_and(|text| offset > text.as_ref().chars().count());
        if starts_past_end {
            return Err(BudgetError::ForeignCursor);
        }
        let mut left = self.chars;
        let mut slices = Vec::new();
        while let Some(text) = units.get(unit).map(AsRef::as_ref) {
            let remaining = text.chars().count() - offset;
            if remaining <= left {
                slices.push(Slice {
                    unit,
                    offset,
                    text: chars_from(text, offset, remaining),
                });
                left -= remaining;
                unit += 1;
                offset = 0;
                continue;
            }
            let has_taken_nothing = left == self.chars;
            if left > 0 && (has_taken_nothing || offset + left >= head(unit)) {
                slices.push(Slice {
                    unit,
                    offset,
                    text: chars_from(text, offset, left),
                });
                offset += left;
                left = 0;
            }
            let cursor = self.scope.cursor(Position { unit, offset });
            return Ok(Taken {
                slices,
                truncation: Truncation::stopping_at(Some(cursor), self.chars - left),
            });
        }
        Ok(Taken {
            slices,
            truncation: Truncation::whole(self.chars - left),
        })
    }
}

/// The number of characters a `max_chars` argument allows.
///
/// # Errors
/// [`BudgetError::ZeroChars`] and [`BudgetError::TooManyChars`].
pub(crate) fn checked_max_chars(max_chars: Option<u32>) -> Result<usize, BudgetError> {
    match max_chars {
        None => Ok(DEFAULT_MAX_CHARS as usize),
        Some(0) => Err(BudgetError::ZeroChars),
        Some(given) if given > LARGEST_MAX_CHARS => Err(BudgetError::TooManyChars { given }),
        Some(given) => Ok(given as usize),
    }
}

fn chars_from(text: &str, offset: usize, count: usize) -> String {
    text.chars().skip(offset).take(count).collect()
}
