//! The addresses of the library's resources: how each is written, read back and described to a
//! client.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use rmcp::model::ResourceTemplate;

const SCHEME: &str = "quanty://";

/// The type of what every resource holds.
pub(super) const MIME_TYPE: &str = "application/json";

/// Everything but the letters, digits and `-._~` is written as `%XX` inside a segment, so that a
/// title with a space or a slash stays one segment.
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// One thing the library gives as a resource, named by its address.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ResourceUri {
    /// The documents of one media.
    Media { title: String },
    /// A document at a glance.
    Overview { document_id: String },
    /// The sections of a document.
    Outline { document_id: String },
    /// One section of a document, counted from 1.
    Section { document_id: String, index: usize },
    /// One page of a document, counted from 1.
    Page { document_id: String, page: u32 },
    /// The saved insights, takeaways and cheat sheet of a document.
    Insights { document_id: String },
    /// One concept.
    Concept { concept_id: String },
}

impl ResourceUri {
    /// The thing that `uri` names, or `None` when it names nothing: another scheme, a path that
    /// no template has, a blank segment, or a number that is not a whole number.
    pub(super) fn parse(uri: &str) -> Option<ResourceUri> {
        let path = uri.strip_prefix(SCHEME)?;
        let segments = path
            .split('/')
            .map(|segment| {
                let decoded = percent_decode_str(segment).decode_utf8().ok()?;
                (!decoded.trim().is_empty()).then(|| decoded.into_owned())
            })
            .collect::<Option<Vec<String>>>()?;
        let [kind, name, rest @ ..] = segments.as_slice() else {
            return None;
        };
        let name = name.clone();
        match (kind.as_str(), rest) {
            ("media", []) => Some(ResourceUri::Media { title: name }),
            ("concept", []) => Some(ResourceUri::Concept { concept_id: name }),
            ("document", []) => Some(ResourceUri::Overview { document_id: name }),
            ("document", [part]) => match part.as_str() {
                "outline" => Some(ResourceUri::Outline { document_id: name }),
                "insights" => Some(ResourceUri::Insights { document_id: name }),
                _ => None,
            },
            ("document", [part, number]) => match part.as_str() {
                "section" => Some(ResourceUri::Section {
                    document_id: name,
                    index: whole_number(number)?,
                }),
                "page" => Some(ResourceUri::Page {
                    document_id: name,
                    page: whole_number(number)?,
                }),
                _ => None,
            },
            _ => None,
        }
    }

    /// How long a peer may keep what this resource gave: a converted chapter never changes, but
    /// the other resources change when insights are written or a PDF is ingested.
    pub(super) fn ttl_ms(&self) -> u64 {
        match self {
            ResourceUri::Outline { .. }
            | ResourceUri::Section { .. }
            | ResourceUri::Page { .. } => CONVERTED_TTL_MS,
            ResourceUri::Media { .. }
            | ResourceUri::Overview { .. }
            | ResourceUri::Insights { .. }
            | ResourceUri::Concept { .. } => CHANGING_TTL_MS,
        }
    }
}

/// An hour: converted content is never rewritten.
const CONVERTED_TTL_MS: u64 = 3_600_000;

/// A minute: an ingest or `write_insights` can change what these resources give.
pub(super) const CHANGING_TTL_MS: u64 = 60_000;

/// A number written with digits only.
fn whole_number<N: std::str::FromStr>(text: &str) -> Option<N> {
    text.bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| text.parse().ok())
        .flatten()
}

fn encoded(segment: &str) -> String {
    utf8_percent_encode(segment, SEGMENT).to_string()
}

/// The address of the resource that lists the documents of a media.
pub(super) fn media_uri(title: &str) -> String {
    format!("{SCHEME}media/{}", encoded(title))
}

/// The address of the resource that gives a document at a glance.
pub(super) fn document_uri(document_id: &str) -> String {
    format!("{SCHEME}document/{}", encoded(document_id))
}

/// The seven kinds of address, for a client that builds one.
pub(super) fn templates() -> Vec<ResourceTemplate> {
    [
        (
            "media/{title}",
            "media",
            "The documents of one media, as `list_documents` gives them. Give the title of the media.",
        ),
        (
            "document/{id}",
            "document",
            "A document at a glance, as `document_overview` gives it: insights, takeaways, concepts and related documents.",
        ),
        (
            "document/{id}/outline",
            "document_outline",
            "The sections of a document with their pages, as `document_outline` gives them.",
        ),
        (
            "document/{id}/section/{n}",
            "document_section",
            "One section of a document as Markdown, as `read_section` gives it. Give the `index` of a section of the outline.",
        ),
        (
            "document/{id}/page/{n}",
            "document_page",
            "The pieces of one page of a document, as `read_page` gives them. Pages count from 1.",
        ),
        (
            "document/{id}/insights",
            "document_insights",
            "The saved insights, takeaways and cheat sheet of a document, as `chapter_insights` gives them.",
        ),
        (
            "concept/{id}",
            "concept",
            "One concept with its definition, aliases and documents, as `concept` gives it.",
        ),
    ]
    .into_iter()
    .map(|(path, name, description)| {
        ResourceTemplate::new(format!("{SCHEME}{path}"), name)
            .with_description(description)
            .with_mime_type(MIME_TYPE)
    })
    .collect()
}
