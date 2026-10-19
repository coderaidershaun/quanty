//! Cuts a section into the passages that one text is written from, so that no model is asked to
//! rewrite more than it writes well.

use crate::section::{Section, estimated_tokens};

/// The most of a section's own text that one draft is written from, so that a rewrite of about
/// the same length stays a size the models write well. The section's notes come on top.
const PASSAGE_TOKENS: usize = 2_500;
const SOURCE_OPEN: &str = "<source>\n";
const SOURCE_CLOSE: &str = "\n</source>";
const NOTES_OPEN: &str = "\n\nNotes:\n";
const PARAGRAPH_SEPARATOR: &str = "\n\n";
const MIN_FENCE_TICKS: usize = 3;
const FORMULA_FENCE: &str = "$$";
/// Where a figure's note starts to say what a model saw in the picture. The note can hold empty
/// lines.
// SMELL: a copy of the words that the section renderer writes into every figure's note. If the
// renderer changes them, a figure's note is cut in two like plain text, and no error says so.
const FIGURE_NOTE_MARK: &str = " — description (model-written): ";

/// The passages of a section in reading order, each as the models read it: the section's own
/// text, then its notes, between `<source>` and `</source>`. A section gives at least one.
pub(super) fn passages_of(section: &Section) -> Vec<String> {
    let notes = if section.notes.trim().is_empty() {
        String::new()
    } else {
        format!("{NOTES_OPEN}{}", section.notes)
    };
    bodies_of(&section.markdown)
        .into_iter()
        .map(|body| format!("{SOURCE_OPEN}{body}{notes}{SOURCE_CLOSE}"))
        .collect()
}

/// The section's own text in a passage: without the tags around it and without the notes. A line
/// that is only "Notes:" after an empty line in the section's own text ends the body early.
pub(super) fn body_of(passage: &str) -> &str {
    let inside = passage.strip_prefix(SOURCE_OPEN).unwrap_or(passage);
    match inside.split_once(NOTES_OPEN) {
        Some((body, _notes)) => body,
        None => inside.strip_suffix(SOURCE_CLOSE).unwrap_or(inside),
    }
}

/// The section's text in one body, or in several of about the same size when it is longer than a
/// passage may be. Every body starts with the heading line.
fn bodies_of(markdown: &str) -> Vec<String> {
    if estimated_tokens(markdown) <= PASSAGE_TOKENS {
        return vec![markdown.to_owned()];
    }
    let mut paragraphs = paragraphs_of(markdown).into_iter();
    let Some(heading) = paragraphs.next() else {
        return vec![markdown.to_owned()];
    };
    let paragraphs: Vec<String> = paragraphs.collect();
    if paragraphs.is_empty() {
        return vec![markdown.to_owned()];
    }
    let heading = format!("{heading}{PARAGRAPH_SEPARATOR}");
    let budget = PASSAGE_TOKENS
        .saturating_sub(estimated_tokens(&heading))
        .max(1);
    let rest = estimated_tokens(&paragraphs.join(PARAGRAPH_SEPARATOR));
    let parts = rest.div_ceil(budget);
    let target = rest.div_ceil(parts);

    let mut bodies: Vec<String> = Vec::new();
    for paragraph in paragraphs {
        match bodies.last_mut() {
            Some(body)
                if estimated_tokens(body) < target
                    && estimated_tokens(&format!("{body}{PARAGRAPH_SEPARATOR}{paragraph}"))
                        <= budget =>
            {
                body.push_str(PARAGRAPH_SEPARATOR);
                body.push_str(&paragraph);
            }
            _ => bodies.push(paragraph),
        }
    }
    bodies
        .into_iter()
        .map(|body| format!("{heading}{body}"))
        .collect()
}

/// The paragraphs of a text. A paragraph ends at an empty line, except inside a code listing, a
/// display formula or the note of a figure, where an empty line belongs to the block.
fn paragraphs_of(markdown: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut lines: Vec<&str> = Vec::new();
    let mut open = Open::Nothing;
    for line in markdown.split('\n') {
        if open == Open::Nothing && line.trim().is_empty() {
            if !lines.is_empty() {
                paragraphs.push(lines.join("\n"));
                lines.clear();
            }
            continue;
        }
        open = open.after(line);
        lines.push(line);
    }
    if !lines.is_empty() {
        paragraphs.push(lines.join("\n"));
    }
    paragraphs
}

/// The block that the lines read so far are inside, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open {
    Nothing,
    /// A code listing, fenced by this many backticks.
    Listing(usize),
    Formula,
    Figure,
}

impl Open {
    /// Where the text stands after this line. Inside a block only its closing line counts. Outside
    /// all blocks a line may open one: a listing first, then a formula, then a figure.
    fn after(self, line: &str) -> Open {
        let trimmed = line.trim();
        match self {
            Open::Listing(ticks) if is_closing_fence(trimmed, ticks) => Open::Nothing,
            Open::Formula if trimmed == FORMULA_FENCE => Open::Nothing,
            Open::Figure if trimmed.ends_with(']') => Open::Nothing,
            Open::Nothing => Open::opened_by(line),
            inside => inside,
        }
    }

    fn opened_by(line: &str) -> Open {
        let ticks = line.trim_start().chars().take_while(|&c| c == '`').count();
        if ticks >= MIN_FENCE_TICKS {
            Open::Listing(ticks)
        } else if line.trim() == FORMULA_FENCE {
            Open::Formula
        } else if line.starts_with('[')
            && line.contains(FIGURE_NOTE_MARK)
            && !line.trim().ends_with(']')
        {
            Open::Figure
        } else {
            Open::Nothing
        }
    }
}

fn is_closing_fence(trimmed: &str, ticks: usize) -> bool {
    trimmed.len() >= ticks && trimmed.chars().all(|c| c == '`')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_stay_whole_across_empty_lines() {
        let figure = "[Figure 1 — description (model-written): A line graph.\n\nIt rises.\n\nThe axis reads 0 to 5.]";
        let one_line_figure = "[Figure 2 — description (model-written): A bar.]";
        let listing = "````\n```\nnot a close\n\n$$\n````";
        let formula = "$$\na = b\n\n```\nc = d\n$$";
        let text = [
            "# Heading",
            figure,
            one_line_figure,
            "plain",
            listing,
            formula,
            "end",
        ]
        .join("\n\n");

        assert_eq!(
            paragraphs_of(&text),
            [
                "# Heading",
                figure,
                one_line_figure,
                "plain",
                listing,
                formula,
                "end"
            ]
        );
    }
}
