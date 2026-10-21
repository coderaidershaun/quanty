//! A document as one HTML file that stands alone: one inline style sheet, pictures and formulas
//! inside the file as data addresses, nothing loaded from outside and nothing run.

use pulldown_cmark::{CowStr, Event, HeadingLevel, Tag, TagEnd, html::push_html};

use crate::commonmark;
use crate::document::language_word;
use crate::math::{Display, Formulas};
use crate::pictures::{Picture, data_uri};
use crate::{Block, Document, Source};

/// Fonts are named and not carried: a reader without them gets a serif or a monospace font of
/// their own.
const STYLE: &str = r#"
body { margin: 0; background: #fff; color: #1c1c1c; font: 17px/1.6 Literata, "Iowan Old Style", Georgia, serif; }
article { max-width: 42rem; margin: 0 auto; padding: 3rem 1.25rem 4rem; }
h1 { margin: 0 0 .5rem; font-size: 1.9rem; line-height: 1.25; }
h2 { margin: 2rem 0 .5rem; font-size: 1.3rem; line-height: 1.3; }
p { margin: 0 0 1rem; }
.origin { margin-bottom: 2rem; color: #666; font-style: italic; }
a.cite { color: #666; text-decoration: none; }
code, pre { font-family: "Source Code Pro", ui-monospace, Menlo, monospace; }
code { font-size: .92em; }
pre { margin: 0; padding: .75rem 1rem; overflow-x: auto; border-radius: 4px; background: #f4f4f4; font-size: .85rem; line-height: 1.45; }
pre code { font-size: inherit; }
img.math { font-size: 1.12em; }
.display-math { display: block; margin: 1rem 0; text-align: center; }
.display-math img.math, figure.formula img.math { max-width: 100%; font-size: 1.25em; }
.points { margin: 0 0 1rem; padding-left: 1.4rem; }
.points li { margin-bottom: .25rem; }
table { border-collapse: collapse; margin: 0 auto; font-size: .95rem; line-height: 1.4; }
th, td { padding: .2rem .8rem; text-align: left; }
thead tr { border-top: 2px solid #444; border-bottom: 1px solid #444; }
tbody tr:last-child { border-bottom: 2px solid #444; }
figure { margin: 1.5rem 0; }
figure > img { display: block; max-width: 100%; height: auto; margin: 0 auto; }
figcaption { margin-top: .5rem; color: #666; font-size: .9rem; font-style: italic; text-align: center; }
figure.table figcaption { margin: 0 0 .5rem; }
figure.formula { text-align: center; }
figure.formula .label { float: right; color: #666; }
figure.code { position: relative; }
figure.code .language { position: absolute; top: .3rem; right: .6rem; color: #666; font-size: .75rem; }
.sources ul { margin: 0; padding-left: 0; list-style: none; font-size: .95rem; }
.sources li { margin-bottom: .3rem; padding-left: 2rem; text-indent: -2rem; }
"#;

pub(crate) fn render(document: &Document) -> String {
    let mut page = Page {
        formulas: Formulas::default(),
        sources: &document.sources,
        markdown_read: 0,
    };
    let title = escape(&document.title);
    let mut body = format!("<h1>{title}</h1>\n");
    if !document.origin.is_empty() {
        body.push_str(&format!(
            "<p class=\"origin\">{}</p>\n",
            escape(&document.origin)
        ));
    }
    for block in &document.blocks {
        body.push_str(&page.block(block));
    }
    if !document.sources.is_empty() {
        body.push_str(&page.sources());
    }
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<article>\n{body}</article>\n</body>\n</html>\n"
    )
}

/// What the file is made of while it is written.
struct Page<'a> {
    formulas: Formulas,
    sources: &'a [Source],
    /// How many pieces of Markdown have been read so far.
    markdown_read: usize,
}

impl Page<'_> {
    fn block(&mut self, block: &Block) -> String {
        match block {
            Block::Heading(heading) => format!("<h2>{}</h2>\n", escape(heading)),
            Block::Text { markdown, cites } => self.paragraphs(markdown, cites),
            Block::List(points) => {
                let items: String = points
                    .iter()
                    .map(|point| format!("<li>{}</li>\n", self.inline(point, &[])))
                    .collect();
                format!("<ul class=\"points\">\n{items}</ul>\n")
            }
            Block::Formula {
                latex,
                label,
                caption,
                cite,
            } => {
                let label = label
                    .as_deref()
                    .map(|label| format!("<span class=\"label\">{}</span>", escape(label)))
                    .unwrap_or_default();
                let formula = self.math(latex, Display::Block);
                format!(
                    "<figure class=\"formula\">{formula}{label}{}</figure>\n",
                    self.figcaption(caption, *cite)
                )
            }
            Block::Figure {
                picture,
                description,
                caption,
                cite,
            } => {
                let shown = match picture.as_deref().and_then(Picture::read) {
                    Some(picture) => format!(
                        "<img src=\"{}\" alt=\"{}\">",
                        picture.data_uri(),
                        escape(description)
                    ),
                    None if description.is_empty() => String::new(),
                    None => format!("<p class=\"description\">{}</p>", escape(description)),
                };
                format!(
                    "<figure>{shown}{}</figure>\n",
                    self.figcaption(caption, *cite)
                )
            }
            Block::Table {
                markdown,
                caption,
                cite,
            } => format!(
                "<figure class=\"table\">{}{}</figure>\n",
                self.figcaption(caption, *cite),
                self.paragraphs(markdown, &[])
            ),
            Block::Code {
                code,
                language,
                caption,
                cite,
            } => {
                let language = language_word(language.as_deref());
                let (tag, name) = if language.is_empty() {
                    (String::new(), "<code>".to_owned())
                } else {
                    let language = escape(language);
                    (
                        format!("<div class=\"language\">{language}</div>"),
                        format!("<code class=\"language-{language}\">"),
                    )
                };
                format!(
                    "<figure class=\"code\">{tag}<pre>{name}{}</code></pre>{}</figure>\n",
                    escape(code),
                    self.figcaption(caption, *cite)
                )
            }
        }
    }

    fn sources(&self) -> String {
        let items: String = self
            .sources
            .iter()
            .map(|source| {
                format!(
                    "<li id=\"source-{n}\">[{n}] {}</li>\n",
                    escape(&source.text),
                    n = source.number
                )
            })
            .collect();
        format!("<section class=\"sources\"><h2>Sources</h2>\n<ul>\n{items}</ul>\n</section>\n")
    }

    /// Empty when there is neither a caption nor a source to name.
    fn figcaption(&mut self, caption: &str, cite: Option<usize>) -> String {
        let cites: Vec<usize> = cite.into_iter().collect();
        let inline = self.inline(caption, &cites);
        if inline.trim().is_empty() {
            return String::new();
        }
        format!("<figcaption>{inline}</figcaption>")
    }

    /// Markdown as blocks. The marks of `cites` go inside its last paragraph, or in a paragraph
    /// of their own when it does not end with one.
    fn paragraphs(&mut self, markdown: &str, cites: &[usize]) -> String {
        let mut events = self.events(markdown, false);
        if !cites.is_empty() {
            let marks = self.cite_marks(cites);
            match events.last() {
                Some(Event::End(TagEnd::Paragraph)) => {
                    let at = events.len() - 1;
                    events.insert(at, Event::InlineHtml(marks.into()));
                }
                _ => events.push(Event::Html(
                    format!("<p class=\"cites\">{}</p>\n", marks.trim_start()).into(),
                )),
            }
        }
        let mut html = String::new();
        push_html(&mut html, events.into_iter());
        html
    }

    /// Markdown with no paragraph around it, for a point of a list or a caption.
    fn inline(&mut self, markdown: &str, cites: &[usize]) -> String {
        let mut events = self.events(markdown, true);
        events.push(Event::InlineHtml(self.cite_marks(cites).into()));
        let mut html = String::new();
        push_html(&mut html, events.into_iter());
        html.trim().to_owned()
    }

    /// The Markdown as events that can only write text, a formula picture or a mark of the
    /// document itself: raw HTML is shown as text, and a link or a picture of the Markdown is
    /// shown as its words, so a file never reaches outside itself. A first-level heading is read
    /// as a second-level one, under the title of the file.
    fn events<'m>(&mut self, markdown: &'m str, bare: bool) -> Vec<Event<'m>> {
        self.markdown_read += 1;
        let piece = self.markdown_read;
        // Each piece of Markdown numbers its notes from one, so their ids name the piece too.
        let note_id = |label: &str| -> CowStr<'static> { format!("note-{piece}-{label}").into() };
        let mut events = Vec::new();
        for event in commonmark::parse(markdown) {
            match event {
                Event::FootnoteReference(label) => {
                    events.push(Event::FootnoteReference(note_id(&label)));
                }
                Event::Start(Tag::FootnoteDefinition(label)) => {
                    events.push(Event::Start(Tag::FootnoteDefinition(note_id(&label))));
                }
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H1,
                    id,
                    classes,
                    attrs,
                }) => events.push(Event::Start(Tag::Heading {
                    level: HeadingLevel::H2,
                    id,
                    classes,
                    attrs,
                })),
                Event::End(TagEnd::Heading(HeadingLevel::H1)) => {
                    events.push(Event::End(TagEnd::Heading(HeadingLevel::H2)));
                }
                Event::InlineMath(latex) => {
                    events.push(Event::InlineHtml(self.math(&latex, Display::Inline).into()));
                }
                Event::DisplayMath(latex) => events.push(Event::Html(
                    format!(
                        "<span class=\"display-math\">{}</span>",
                        self.math(&latex, Display::Block)
                    )
                    .into(),
                )),
                Event::Html(text) | Event::InlineHtml(text) => events.push(Event::Text(text)),
                Event::Start(Tag::Image { .. } | Tag::Link { .. })
                | Event::End(TagEnd::Image | TagEnd::Link) => {}
                Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) if bare => {}
                event => events.push(event),
            }
        }
        events
    }

    /// A picture of the formula, sized in em so that it grows with the text around it. The LaTeX
    /// is its alternative text, and the page shows it as code when the formula cannot be
    /// typeset.
    fn math(&mut self, latex: &str, display: Display) -> String {
        let Some(formula) = self.formulas.get(latex, display) else {
            return format!("<code class=\"math\">{}</code>", escape(latex));
        };
        // A formula set apart is given only its width, so that a wide one shrinks to the page and
        // keeps its shape. A formula in a line also needs its height and its place on the baseline.
        let style = match display {
            Display::Block => format!("width:{:.4}em", formula.picture_width_em()),
            Display::Inline => format!(
                "width:{:.4}em;height:{:.4}em;vertical-align:-{:.4}em;margin:0 -{:.4}em",
                formula.picture_width_em(),
                formula.picture_height_em(),
                formula.descent_em + formula.bleed_em,
                formula.bleed_em,
            ),
        };
        format!(
            "<img class=\"math\" alt=\"{}\" src=\"{}\" style=\"{style}\">",
            escape(latex),
            data_uri("image/png", &formula.png),
        )
    }

    /// ` [1] [2]`, each mark a link to its entry in the list of sources when the list has it.
    fn cite_marks(&self, cites: &[usize]) -> String {
        cites
            .iter()
            .map(|&number| {
                if self.sources.iter().any(|source| source.number == number) {
                    format!(" <a class=\"cite\" href=\"#source-{number}\">[{number}]</a>")
                } else {
                    format!(" [{number}]")
                }
            })
            .collect()
    }
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}
