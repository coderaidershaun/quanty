//! Markdown that a model wrote, with its formulas between dollar signs, read into the blocks that
//! rich text draws: headings, text, formulas set apart and tables. Each formula of a text, a
//! heading or a table cell is written between `\(` and `\)`, as stored text has it.

/// One block of saved Markdown, in the kind that rich text, a formula or a table draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DollarBlock {
    /// The words of a `#` line, without the marks.
    Heading(String),
    /// Paragraphs and list lines, read as one block of rich text.
    Text(String),
    /// A formula set apart between `$$`: its LaTeX, with no delimiters.
    Formula(String),
    /// A GitHub table, one line for each row.
    Table(String),
}

/// Reads `markdown` line by line. A blank line ends the text or table being gathered, a table
/// ends at the first line that does not start with a bar, and a formula that is opened with `$$`
/// and never closed before the next blank line is text.
pub fn dollar_blocks(markdown: &str) -> Vec<DollarBlock> {
    let lines: Vec<&str> = markdown.lines().map(str::trim).collect();
    let mut reader = Reader::default();
    let mut at = 0;
    while at < lines.len() {
        let line = lines[at];
        at += 1;
        if line.is_empty() {
            reader.end_text();
            reader.end_table();
        } else if line.starts_with('|') {
            reader.end_text();
            reader.table.push(line);
        } else {
            reader.end_table();
            if let Some(words) = heading_words(line) {
                reader.end_text();
                reader
                    .blocks
                    .push(DollarBlock::Heading(bracket_formulas(words)));
            } else if let Some(latex) = formula_on_one_line(line) {
                reader.end_text();
                reader.blocks.push(DollarBlock::Formula(latex.to_owned()));
            } else if let Some((latex, lines_used)) = formula_on_many_lines(line, &lines[at..]) {
                reader.end_text();
                reader.blocks.push(DollarBlock::Formula(latex));
                at += lines_used;
            } else {
                reader.text.push(line);
            }
        }
    }
    reader.end_text();
    reader.end_table();
    reader.blocks
}

/// The blocks read so far, and the lines of the text and of the table that are still being
/// gathered.
#[derive(Default)]
struct Reader<'a> {
    blocks: Vec<DollarBlock>,
    text: Vec<&'a str>,
    table: Vec<&'a str>,
}

impl Reader<'_> {
    fn end_text(&mut self) {
        if !self.text.is_empty() {
            let text = bracket_formulas(&self.text.join("\n"));
            self.blocks.push(DollarBlock::Text(text));
            self.text.clear();
        }
    }

    fn end_table(&mut self) {
        if !self.table.is_empty() {
            let rows = bracket_formulas(&self.table.join("\n"));
            self.blocks.push(DollarBlock::Table(rows));
            self.table.clear();
        }
    }
}

/// The words of a `#` line: one to six marks, a space, then the words.
fn heading_words(line: &str) -> Option<&str> {
    let words = line.trim_start_matches('#');
    let marks = line.len() - words.len();
    ((1..=6).contains(&marks) && words.starts_with(' ')).then(|| words.trim())
}

/// The LaTeX of a line that is `$$…$$` and nothing else.
fn formula_on_one_line(line: &str) -> Option<&str> {
    let latex = line.strip_prefix("$$")?.strip_suffix("$$")?;
    (!latex.contains("$$") && !latex.trim().is_empty()).then(|| latex.trim())
}

/// The LaTeX of a formula that `line` opens with `$$` and a later line of `following` closes, and
/// how many of `following` it took. The closing line is the first one that ends with `$$`. A
/// blank line before it leaves the formula open.
fn formula_on_many_lines(line: &str, following: &[&str]) -> Option<(String, usize)> {
    let opening = line
        .strip_prefix("$$")
        .filter(|rest| !rest.contains("$$"))?;
    let closing = following
        .iter()
        .take_while(|next| !next.is_empty())
        .position(|next| next.ends_with("$$"))?;
    let last = following[closing].strip_suffix("$$")?;
    let middle = &following[..closing];
    let latex = std::iter::once(opening)
        .chain(middle.iter().copied())
        .chain(std::iter::once(last))
        .collect::<Vec<_>>()
        .join("\n");
    let latex = latex.trim();
    (!latex.is_empty()).then(|| (latex.to_owned(), closing + 1))
}

/// `text` with `$…$` and `$$…$$` written as `\(…\)`, `\$` as a dollar sign, and code spans as they
/// are. A dollar sign that opens no formula stays as it is.
fn bracket_formulas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut bracketed = String::with_capacity(text.len());
    let mut at = 0;
    while at < chars.len() {
        match chars[at] {
            '`' => match chars[at + 1..].iter().position(|&c| c == '`') {
                Some(offset) => {
                    let end = at + 1 + offset;
                    bracketed.extend(&chars[at..=end]);
                    at = end + 1;
                }
                None => {
                    bracketed.push('`');
                    at += 1;
                }
            },
            '\\' => match chars.get(at + 1) {
                Some('$') => {
                    bracketed.push('$');
                    at += 2;
                }
                Some(&escaped) => {
                    bracketed.push('\\');
                    bracketed.push(escaped);
                    at += 2;
                }
                None => {
                    bracketed.push('\\');
                    at += 1;
                }
            },
            '$' => match formula_at(&chars, at) {
                Some((latex, length)) => {
                    bracketed.push_str("\\(");
                    bracketed.push_str(&latex);
                    bracketed.push_str("\\)");
                    at += length;
                }
                None => {
                    bracketed.push('$');
                    at += 1;
                }
            },
            other => {
                bracketed.push(other);
                at += 1;
            }
        }
    }
    bracketed
}

/// The formula that the dollar sign at `chars[at]` opens: its LaTeX and how many characters it
/// takes. Two signs open a formula that the next two close. One sign opens a formula when no
/// white space follows it, and the next sign that is not escaped closes it when no white space
/// comes before that sign. These are the rules of the reader that writes the exported files.
fn formula_at(chars: &[char], at: usize) -> Option<(String, usize)> {
    if chars.get(at + 1) == Some(&'$') {
        let close = (at + 2..chars.len().saturating_sub(1))
            .find(|&i| chars[i] == '$' && chars[i + 1] == '$')?;
        let latex: String = chars[at + 2..close].iter().collect();
        let latex = latex.trim();
        return (!latex.is_empty()).then(|| (latex.to_owned(), close + 2 - at));
    }
    if chars.get(at + 1)?.is_ascii_whitespace() {
        return None;
    }
    let mut close = at + 1;
    while close < chars.len() && chars[close] != '$' {
        close += if chars[close] == '\\' { 2 } else { 1 };
    }
    if close >= chars.len() || chars[close - 1].is_ascii_whitespace() {
        return None;
    }
    Some((chars[at + 1..close].iter().collect(), close + 1 - at))
}
