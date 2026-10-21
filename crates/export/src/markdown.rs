//! A document as a Markdown file: the text as it was given, formulas between dollar signs,
//! pictures inside the file as data addresses.

use crate::commonmark;
use crate::document::language_word;
use crate::pictures::Picture;
use crate::{Block, Document};

/// Three backticks, or one more than the longest run of them in the code, so that a listing that
/// holds a fence of its own cannot end the block early.
pub fn fence_for(code: &str) -> String {
    let longest_run = code
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    "`".repeat(longest_run.max(2) + 1)
}

pub(crate) fn render(document: &Document) -> String {
    let mut parts = vec![format!("# {}", one_line(&document.title))];
    if !document.origin.is_empty() {
        parts.push(format!("> {}", one_line(&document.origin)));
    }
    for block in &document.blocks {
        push_block(block, &mut parts);
    }
    if !document.sources.is_empty() {
        let lines: Vec<String> = document
            .sources
            .iter()
            .map(|source| format!("- [{}] {}", source.number, one_line(&source.text)))
            .collect();
        parts.push(format!("## Sources\n\n{}", lines.join("\n")));
    }
    let mut text = parts.join("\n\n");
    text.push('\n');
    text
}

fn push_block(block: &Block, parts: &mut Vec<String>) {
    match block {
        Block::Heading(heading) => parts.push(format!("## {}", one_line(heading))),
        // SMELL: each text is written as it was given, so two texts that use the same footnote
        // label give the file two notes of that name, and a reader shows the first one for both.
        Block::Text { markdown, cites } => {
            let text = markdown.trim_end();
            let marks: String = cites.iter().map(|number| format!(" [{number}]")).collect();
            if marks.is_empty() || commonmark::ends_with_paragraph(text) {
                parts.push(format!("{text}{marks}"));
            } else {
                // Marks written on the last line of a table or a code fence would become part of
                // it, so they stand as a paragraph of their own.
                if !text.is_empty() {
                    parts.push(text.to_owned());
                }
                parts.push(marks.trim_start().to_owned());
            }
        }
        Block::List(points) => {
            let lines: Vec<String> = points
                .iter()
                .map(|point| format!("- {}", one_line(point)))
                .collect();
            parts.push(lines.join("\n"));
        }
        Block::Formula {
            latex,
            label,
            caption,
            cite,
        } => {
            parts.push(format!("$$\n{}\n$$", latex.trim()));
            parts.extend(caption_line(label.as_deref(), caption, *cite));
        }
        Block::Figure {
            picture,
            description,
            caption,
            cite,
        } => {
            match picture.as_deref().and_then(Picture::read) {
                Some(picture) => {
                    let alt: String = description
                        .chars()
                        .filter(|character| !matches!(character, '[' | ']'))
                        .collect();
                    parts.push(format!("![{}]({})", one_line(&alt), picture.data_uri()));
                }
                None if !description.is_empty() => parts.push(description.trim().to_owned()),
                None => {}
            }
            parts.extend(caption_line(None, caption, *cite));
        }
        Block::Table {
            markdown,
            caption,
            cite,
        } => {
            parts.extend(caption_line(None, caption, *cite));
            parts.push(markdown.trim().to_owned());
        }
        Block::Code {
            code,
            language,
            caption,
            cite,
        } => {
            let fence = fence_for(code);
            let language = language_word(language.as_deref());
            parts.push(format!(
                "{fence}{language}\n{}\n{fence}",
                code.trim_end_matches('\n')
            ));
            parts.extend(caption_line(None, caption, *cite));
        }
    }
}

/// The label, the caption and the mark of the source that a block is printed with, as one line.
/// `None` when the block has none of them.
fn caption_line(label: Option<&str>, caption: &str, cite: Option<usize>) -> Option<String> {
    let words: Vec<String> = label
        .filter(|label| !label.is_empty())
        .map(str::to_owned)
        .into_iter()
        .chain((!caption.is_empty()).then(|| caption.to_owned()))
        .chain(cite.map(|number| format!("[{number}]")))
        .collect();
    (!words.is_empty()).then(|| one_line(&words.join(" ")))
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
