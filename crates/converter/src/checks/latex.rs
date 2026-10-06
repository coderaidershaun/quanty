//! A small reader for LaTeX that finds the mistakes a model makes when it writes math inside
//! JSON: lost backslashes, unbalanced braces, and signs that would cut a formula short.
//!
//! Text is read left to right and a backslash together with the character after it is one unit.
//! So `\{` is an escaped brace, `\\` is a row break, and `\\{` is a row break followed by an
//! opening brace.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind<'a> {
    /// A backslash and the letters after it, such as `\left`. Holds the letters.
    Command(&'a str),
    /// A backslash and the one character after it that is not a letter.
    Escaped(char),
    Plain(char),
}

#[derive(Debug, Clone, Copy)]
struct Unit<'a> {
    start: usize,
    end: usize,
    kind: Kind<'a>,
}

fn units(text: &str) -> Vec<Unit<'_>> {
    let mut units = Vec::new();
    let mut characters = text.char_indices().peekable();
    while let Some((start, character)) = characters.next() {
        let mut end = start + character.len_utf8();
        let kind = if character != '\\' {
            Kind::Plain(character)
        } else {
            match characters.peek().copied() {
                None => Kind::Plain('\\'),
                Some((_, next)) if next.is_ascii_alphabetic() => {
                    while let Some(&(index, letter)) = characters.peek() {
                        if !letter.is_ascii_alphabetic() {
                            break;
                        }
                        end = index + letter.len_utf8();
                        characters.next();
                    }
                    Kind::Command(&text[start + 1..end])
                }
                Some((index, next)) => {
                    end = index + next.len_utf8();
                    characters.next();
                    Kind::Escaped(next)
                }
            }
        };
        units.push(Unit { start, end, kind });
    }
    units
}

/// The name in a `\begin{name}` or `\end{name}` whose command is the unit at `command_index`.
fn environment_name<'a>(
    text: &'a str,
    units: &[Unit<'_>],
    command_index: usize,
) -> Option<&'a str> {
    let open = units.get(command_index + 1)?;
    if open.kind != Kind::Plain('{') {
        return None;
    }
    let close = units[command_index + 2..]
        .iter()
        .find(|unit| unit.kind == Kind::Plain('}'))?;
    Some(&text[open.end..close.start])
}

/// The text inside each `\( … \)` of `text`, or what is wrong with how the spans are opened and
/// closed.
pub(crate) fn math_spans(text: &str) -> Result<Vec<&str>, &'static str> {
    let mut spans = Vec::new();
    let mut open_at = None;
    for unit in units(text) {
        match (unit.kind, open_at) {
            (Kind::Escaped('('), None) => open_at = Some(unit.end),
            (Kind::Escaped('('), Some(_)) => return Err("a \\( opened inside another \\("),
            (Kind::Escaped(')'), Some(start)) => {
                spans.push(&text[start..unit.start]);
                open_at = None;
            }
            (Kind::Escaped(')'), None) => return Err("a \\) with no \\( before it"),
            _ => {}
        }
    }
    match open_at {
        Some(_) => Err("a \\( with no \\) after it"),
        None => Ok(spans),
    }
}

/// What is unbalanced in `latex`: braces, `\begin` and `\end` by name, `\left` and `\right`.
pub(crate) fn unbalanced(latex: &str) -> Option<&'static str> {
    let units = units(latex);
    let mut depth = 0_i32;
    let mut open_environments = Vec::new();
    let (mut lefts, mut rights) = (0, 0);
    for (index, unit) in units.iter().enumerate() {
        match unit.kind {
            Kind::Plain('{') => depth += 1,
            Kind::Plain('}') => {
                depth -= 1;
                if depth < 0 {
                    return Some("a closing brace with no opening brace");
                }
            }
            Kind::Command("left") => lefts += 1,
            Kind::Command("right") => rights += 1,
            Kind::Command("begin") => {
                if let Some(name) = environment_name(latex, &units, index) {
                    open_environments.push(name);
                }
            }
            Kind::Command("end") => {
                if let Some(name) = environment_name(latex, &units, index)
                    && open_environments.pop() != Some(name)
                {
                    return Some("an \\end with no \\begin of the same name before it");
                }
            }
            _ => {}
        }
    }
    if depth > 0 {
        return Some("an opening brace with no closing brace");
    }
    if !open_environments.is_empty() {
        return Some("a \\begin with no \\end of the same name after it");
    }
    if lefts != rights {
        return Some("a different number of \\left and \\right commands");
    }
    None
}

/// A `%` or `$` with no backslash before it. In LaTeX a bare `%` starts a comment and a bare `$`
/// opens or closes math.
pub(crate) fn bare_percent_or_dollar(latex: &str) -> Option<&'static str> {
    units(latex).iter().find_map(|unit| match unit.kind {
        Kind::Plain('%') => {
            Some("it holds a % with no backslash before it; a printed percent sign is written \\%")
        }
        Kind::Plain('$') => {
            Some("it holds a $ with no backslash before it; a printed dollar sign is written \\$")
        }
        _ => None,
    })
}

const WRAPPING_ENVIRONMENTS: [&str; 8] = [
    "equation",
    "equation*",
    "align",
    "align*",
    "gather",
    "gather*",
    "multline",
    "multline*",
];

/// Why a formula's LaTeX would not typeset as the formula it is: it is wrapped in delimiters or
/// an outer environment, or carries its own label.
pub(crate) fn wrapped_or_labelled(latex: &str) -> Option<&'static str> {
    let latex = latex.trim_start();
    if latex.starts_with("\\(") {
        return Some("the LaTeX is wrapped in \\( and \\); write the LaTeX alone");
    }
    if latex.starts_with("\\[") {
        return Some("the LaTeX is wrapped in \\[ and \\]; write the LaTeX alone");
    }
    if latex.starts_with('$') {
        return Some("the LaTeX is wrapped in dollar signs; write the LaTeX alone");
    }
    let units = units(latex);
    if units.first().map(|unit| unit.kind) == Some(Kind::Command("begin"))
        && environment_name(latex, &units, 0)
            .is_some_and(|name| WRAPPING_ENVIRONMENTS.contains(&name))
    {
        return Some(
            "the LaTeX is wrapped in an equation or align environment; write the LaTeX alone",
        );
    }
    units
        .iter()
        .any(|unit| matches!(unit.kind, Kind::Command("tag" | "label")))
        .then_some("the LaTeX holds a \\tag or \\label; the label belongs in the label field")
}

/// True when `latex` opens an `aligned` block and never writes a row break.
pub(crate) fn aligned_without_row_break(latex: &str) -> bool {
    let units = units(latex);
    let opens_aligned = units.iter().enumerate().any(|(index, unit)| {
        unit.kind == Kind::Command("begin")
            && environment_name(latex, &units, index) == Some("aligned")
    });
    opens_aligned && !units.iter().any(|unit| unit.kind == Kind::Escaped('\\'))
}

/// Turns each row break followed by a line break into a row break followed by a space, except
/// where a letter comes next: a lost backslash before a command such as `\nu` looks the same, and
/// must still reach the reply check.
pub(crate) fn mend_row_breaks(latex: &str) -> String {
    let mut mended = String::with_capacity(latex.len());
    let mut resume_at = 0;
    for unit in units(latex) {
        if unit.start < resume_at {
            continue;
        }
        mended.push_str(&latex[unit.start..unit.end]);
        if unit.kind != Kind::Escaped('\\') {
            continue;
        }
        let Some(after_break) = line_break_end(latex, unit.end) else {
            continue;
        };
        if !latex[after_break..]
            .chars()
            .next()
            .is_some_and(char::is_alphabetic)
        {
            mended.push(' ');
            resume_at = after_break;
        }
    }
    mended
}

/// Where the line break that follows `from` (after any spaces) ends, if there is one.
fn line_break_end(text: &str, from: usize) -> Option<usize> {
    let after_spaces = text[from..].trim_start_matches(' ');
    let after_break = after_spaces
        .strip_prefix("\r\n")
        .or_else(|| after_spaces.strip_prefix('\n'))?;
    Some(text.len() - after_break.len())
}

/// True when `latex` has a row break outside any braces and any `\begin` environment. Inside
/// braces one is fine (a stacked sum limit uses one).
pub(crate) fn has_bare_row_break(latex: &str) -> bool {
    let mut depth = 0_i32;
    let mut open_environments = 0_i32;
    let units = units(latex);
    for (index, unit) in units.iter().enumerate() {
        match unit.kind {
            Kind::Plain('{') => depth += 1,
            Kind::Plain('}') => depth -= 1,
            Kind::Command("begin") if environment_name(latex, &units, index).is_some() => {
                open_environments += 1;
            }
            Kind::Command("end") if environment_name(latex, &units, index).is_some() => {
                open_environments -= 1;
            }
            Kind::Escaped('\\') if depth == 0 && open_environments == 0 => return true,
            _ => {}
        }
    }
    false
}
