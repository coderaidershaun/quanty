//! Reads the notes of one tool: a Markdown file with a title, a long description, and the sections
//! Arguments, Examples, Errors, Cost and Related.

use rmcp::model::JsonObject;

use super::error::CatalogError;

/// The notes of a tool, split into their sections.
pub(super) struct Notes<'a> {
    tool: &'a str,
    description: String,
    sections: Vec<(&'a str, Vec<&'a str>)>,
}

impl<'a> Notes<'a> {
    pub(super) fn parse(tool: &'a str, text: &'a str) -> Notes<'a> {
        let mut description = Vec::new();
        let mut sections: Vec<(&str, Vec<&str>)> = Vec::new();
        let mut in_fence = false;
        for line in text.lines().skip(1) {
            if line.starts_with("```") {
                in_fence = !in_fence;
            }
            match line.strip_prefix("## ") {
                Some(heading) if !in_fence => sections.push((heading.trim(), Vec::new())),
                _ => match sections.last_mut() {
                    Some((_, lines)) => lines.push(line),
                    None => description.push(line),
                },
            }
        }
        Notes {
            tool,
            description: description.join("\n").trim().to_owned(),
            sections,
        }
    }

    /// The text under the title, before the first section.
    pub(super) fn description(&self) -> &str {
        &self.description
    }

    /// The text of a section, or an empty text when the notes have none.
    pub(super) fn section(&self, heading: &str) -> String {
        self.lines_of(heading).join("\n").trim().to_owned()
    }

    /// The JSON objects in the code blocks of the Examples section: each is the arguments of one
    /// call.
    ///
    /// # Errors
    /// [`CatalogError::BadExample`] for a block that is not a JSON object.
    pub(super) fn examples(&self) -> Result<Vec<JsonObject>, CatalogError> {
        let mut examples = Vec::new();
        let mut block: Option<Vec<&str>> = None;
        for line in self.lines_of("Examples") {
            match (line.starts_with("```"), block.take()) {
                (true, Some(lines)) => {
                    let example = serde_json::from_str(&lines.join("\n")).map_err(|source| {
                        CatalogError::BadExample {
                            tool: self.tool.to_owned(),
                            source,
                        }
                    })?;
                    examples.push(example);
                }
                (true, None) => block = Some(Vec::new()),
                (false, Some(mut lines)) => {
                    lines.push(line);
                    block = Some(lines);
                }
                (false, None) => {}
            }
        }
        Ok(examples)
    }

    /// The bullets of a section, each on one line.
    pub(super) fn bullets(&self, heading: &str) -> Vec<String> {
        let mut bullets: Vec<String> = Vec::new();
        for line in self.lines_of(heading) {
            if let Some(first) = line.strip_prefix("- ") {
                bullets.push(first.trim().to_owned());
            } else if let (Some(last), false) = (bullets.last_mut(), line.trim().is_empty()) {
                last.push(' ');
                last.push_str(line.trim());
            }
        }
        bullets
    }

    /// The tool names in backticks at the start of the bullets of the Related section.
    pub(super) fn related(&self) -> Vec<String> {
        self.bullets("Related")
            .iter()
            .filter_map(|bullet| {
                bullet
                    .strip_prefix('`')?
                    .split('`')
                    .next()
                    .map(str::to_owned)
            })
            .collect()
    }

    fn lines_of(&self, heading: &str) -> &[&'a str] {
        self.sections
            .iter()
            .find(|(name, _)| *name == heading)
            .map_or(&[], |(_, lines)| lines.as_slice())
    }
}
