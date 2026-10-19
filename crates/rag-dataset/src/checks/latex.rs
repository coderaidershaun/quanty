//! Finds the math in the text of a section and checks that each formula can be read by the
//! typesetter that the app draws formulas with.

/// Whether every formula between dollars in `prose` parses, or none when it holds no math. Math
/// that is opened and never closed does not parse.
pub(super) fn renders(prose: &str) -> Option<bool> {
    let Math { spans, is_unclosed } = math_in(prose);
    if spans.is_empty() && !is_unclosed {
        return None;
    }
    Some(!is_unclosed && spans.iter().all(|span| parses(span)))
}

enum Delimiter {
    /// Math between single dollar signs.
    Inline,
    /// Math between double dollar signs.
    Display,
}

struct Math {
    spans: Vec<String>,
    is_unclosed: bool,
}

/// The text between the dollars of each span. A backslash and the character after it are read
/// together and never end a span, so `\$` is a dollar sign and the dollar after `\\` is math.
/// This is the rule that the Markdown is written by: every `$` without a backslash is math.
fn math_in(prose: &str) -> Math {
    let mut spans = Vec::new();
    let mut open: Option<(Delimiter, String)> = None;
    let mut characters = prose.chars().peekable();
    while let Some(character) = characters.next() {
        match (character, &mut open) {
            ('\\', inside) => {
                let escaped = characters.next();
                if let Some((_, text)) = inside {
                    text.push('\\');
                    text.extend(escaped);
                }
            }
            ('$', None) => {
                let delimiter = if characters.next_if_eq(&'$').is_some() {
                    Delimiter::Display
                } else {
                    Delimiter::Inline
                };
                open = Some((delimiter, String::new()));
            }
            ('$', Some((Delimiter::Inline, _))) => spans.extend(open.take().map(|(_, text)| text)),
            ('$', Some((Delimiter::Display, text))) => {
                if characters.next_if_eq(&'$').is_some() {
                    spans.extend(open.take().map(|(_, text)| text));
                } else {
                    text.push('$');
                }
            }
            (character, Some((_, text))) => text.push(character),
            (_, None) => {}
        }
    }
    Math {
        spans,
        is_unclosed: open.is_some(),
    }
}

fn parses(latex: &str) -> bool {
    // The typesetter is code from another crate. A formula that makes it panic must fail only this
    // check and must not end the whole build.
    matches!(
        std::panic::catch_unwind(|| ratex_parser::parser::parse(latex.trim())),
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use super::renders;

    #[test]
    fn math_that_is_opened_and_never_closed_does_not_render() {
        assert_eq!(renders("fine $x$ then open $y"), Some(false));
    }
}
