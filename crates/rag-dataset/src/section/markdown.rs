//! How one piece is written in Markdown: math between dollars, formulas with their printed
//! labels, fenced listings, the label lines of tables and listings, and figure notes.

use ocr::Symbol;

/// The line the converter adds under a figure's explanation to list the words printed in it.
const PRINTED_TEXT_MARK: &str = "\n\nText printed in the figure: ";

/// The words between a figure's name and its model-written explanation. The cut of a section into
/// passages finds a figure's note by these words, so that it never cuts inside the note.
pub(super) const FIGURE_NOTE_MARK: &str = " — description (model-written): ";

/// The converter marks math with `\(` and `\)`, so a `$` in its text is a dollar sign. That `$` is
/// escaped as `\$`, and then the marks become `$`, so that every `$` left marks math.
pub(super) fn inline_math(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    let mut before = None;
    for character in text.chars() {
        if character == '$' && before != Some('\\') {
            escaped.push('\\');
        }
        escaped.push(character);
        before = Some(character);
    }
    escaped.replace("\\(", "$").replace("\\)", "$")
}

/// A label, caption or name without the white space around it, or none when nothing is left.
pub(super) fn non_blank(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

pub(super) fn heading_text(printed_number: Option<&str>, text: &str) -> String {
    match printed_number {
        Some(number) => format!("{number} {text}"),
        None => text.to_owned(),
    }
}

pub(super) fn formula_text(latex: &str, label: Option<&str>) -> String {
    let mut text = format!("$$\n{}\n$$", latex.trim());
    if let Some(label) = non_blank(label) {
        text.push('\n');
        text.push_str(&parenthesised(label));
    }
    text
}

fn parenthesised(label: &str) -> String {
    if label.starts_with('(') && label.ends_with(')') {
        label.to_owned()
    } else {
        format!("({label})")
    }
}

pub(super) fn formula_note(
    label: Option<&str>,
    name: Option<&str>,
    statement: &str,
    symbols: &[Symbol],
) -> Option<String> {
    let name = non_blank(name);
    let statement = statement.trim();
    if name.is_none() && statement.is_empty() && symbols.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(name) = name {
        parts.push(format!("{}.", name.trim_end_matches('.')));
    }
    if !statement.is_empty() {
        parts.push(statement.to_owned());
    }
    if !symbols.is_empty() {
        let meanings: Vec<String> = symbols
            .iter()
            .map(|symbol| format!("{}: {}", symbol.symbol, symbol.meaning))
            .collect();
        parts.push(format!("Symbols: {}", meanings.join("; ")));
    }
    let label = non_blank(label)
        .map(|label| format!(" {}", parenthesised(label)))
        .unwrap_or_default();
    Some(format!("Formula{label}: {}", parts.join(" ")))
}

/// The label line of a table or a listing, then the table or the listing.
pub(super) fn with_label_line(label: Option<&str>, caption: Option<&str>, body: &str) -> String {
    let [label, caption] = [label, caption].map(|part| non_blank(part).map(inline_math));
    match (label, caption) {
        (Some(label), Some(caption)) => format!("{label}: {caption}\n{body}"),
        (Some(line), None) | (None, Some(line)) => format!("{line}\n{body}"),
        (None, None) => body.to_owned(),
    }
}

/// The listing in a fence that is longer than any run of backticks inside it.
pub(super) fn fenced(listing: &str, language: Option<&str>) -> String {
    let mut longest_run = 0;
    let mut run = 0;
    for character in listing.chars() {
        run = if character == '`' { run + 1 } else { 0 };
        longest_run = longest_run.max(run);
    }
    let fence = "`".repeat((longest_run + 1).max(3));
    format!(
        "{fence}{}\n{listing}\n{fence}",
        language.unwrap_or_default()
    )
}

/// "Figure 13-4" for a label that already says so, "Figure 7" for the label "7", and "Figure" for
/// none.
pub(super) fn figure_name(label: Option<&str>) -> String {
    match non_blank(label) {
        Some(label) if label.to_lowercase().starts_with("figure") => inline_math(label),
        Some(label) => format!("Figure {}", inline_math(label)),
        None => "Figure".to_owned(),
    }
}

/// The piece file of a figure is the model's explanation and then, on a line of its own, the
/// words printed in the figure. Only the explanation is written into the text.
pub(super) fn explanation_of(content: &str) -> &str {
    match content.rsplit_once(PRINTED_TEXT_MARK) {
        Some((explanation, _)) => explanation.trim(),
        None => content.trim(),
    }
}
