//! The prompts of the server: short instructions that name the tools to call, in order, for three
//! jobs that take several of them.

use rmcp::ErrorData;
use rmcp::model::{
    CacheScope, GetPromptRequestParams, GetPromptResult, JsonObject, ListPromptsResult, Prompt,
    PromptArgument, PromptMessage, Role,
};

use super::TOOL_LIST_TTL_MS;

const RESEARCH_TOPIC: &str = "research_topic";
const SUMMARISE_DOCUMENT: &str = "summarise_document";
const INGEST_BOOK: &str = "ingest_book";

fn required(name: &'static str, description: &'static str) -> PromptArgument {
    PromptArgument::new(name)
        .with_description(description)
        .with_required(true)
}

fn optional(name: &'static str, description: &'static str) -> PromptArgument {
    PromptArgument::new(name)
        .with_description(description)
        .with_required(false)
}

fn prompts() -> Vec<Prompt> {
    vec![
        Prompt::new(
            RESEARCH_TOPIC,
            Some("Research a topic across the library and answer with the pages cited."),
            Some(vec![
                required("topic", "The topic or question to research."),
                optional("media", "The title of one media to keep the research to."),
            ]),
        ),
        Prompt::new(
            SUMMARISE_DOCUMENT,
            Some("Summarise one document, citing the pages the summary rests on."),
            Some(vec![required(
                "document_id",
                "The id of the document, from `list_documents` or a search result.",
            )]),
        ),
        Prompt::new(
            INGEST_BOOK,
            Some("Ingest the chapters of a book from a folder, one at a time, with a check first."),
            Some(vec![
                required("media", "The title of the book."),
                required(
                    "folder",
                    "The folder on the machine the server runs on that holds the chapter PDFs.",
                ),
            ]),
        ),
    ]
}

/// The prompts of the server. They are the same for every caller, so a peer may keep them as long
/// as it keeps the list of tools.
pub(super) fn list(sends_cache_hints: bool) -> ListPromptsResult {
    let listed = ListPromptsResult::with_all_items(prompts());
    if sends_cache_hints {
        listed
            .with_ttl_ms(TOOL_LIST_TTL_MS)
            .with_cache_scope(CacheScope::Public)
    } else {
        listed
    }
}

/// The prompt that `request` names, with its arguments filled into the text.
///
/// # Errors
/// An invalid-params error for a prompt that does not exist and for a required argument that is
/// missing, blank or not text.
pub(super) fn get(request: GetPromptRequestParams) -> Result<GetPromptResult, ErrorData> {
    let arguments = request.arguments.unwrap_or_default();
    let text = match request.name.as_str() {
        RESEARCH_TOPIC => research_topic(
            &required_argument(&arguments, "topic")?,
            optional_argument(&arguments, "media")?.as_deref(),
        ),
        SUMMARISE_DOCUMENT => summarise_document(&required_argument(&arguments, "document_id")?),
        INGEST_BOOK => ingest_book(
            &required_argument(&arguments, "media")?,
            &required_argument(&arguments, "folder")?,
        ),
        other => {
            return Err(ErrorData::invalid_params(
                format!(
                    "there is no prompt named {other:?}; the prompts are {RESEARCH_TOPIC}, {SUMMARISE_DOCUMENT} and {INGEST_BOOK}"
                ),
                None,
            ));
        }
    };
    Ok(GetPromptResult::new(vec![PromptMessage::new_text(
        Role::User,
        text,
    )]))
}

/// The text of an argument, or `None` when it is left out or blank.
fn optional_argument(arguments: &JsonObject, name: &str) -> Result<Option<String>, ErrorData> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    let text = value.as_str().ok_or_else(|| {
        ErrorData::invalid_params(format!("the argument `{name}` must be text"), None)
    })?;
    Ok(Some(text.trim().to_owned()).filter(|text| !text.is_empty()))
}

fn required_argument(arguments: &JsonObject, name: &str) -> Result<String, ErrorData> {
    optional_argument(arguments, name)?.ok_or_else(|| {
        ErrorData::invalid_params(format!("the argument `{name}` is required"), None)
    })
}

fn research_topic(topic: &str, media: Option<&str>) -> String {
    let (kept_to, find) = match media {
        Some(media) => (
            format!(" Keep to the media {media:?}."),
            format!(
                "`list_documents` with `media` set to {media:?} (or `search` for the topic) to find the documents"
            ),
        ),
        None => (
            String::new(),
            "`search` for the topic (or `list_documents`) to find the documents".to_owned(),
        ),
    };
    format!(
        "Research this in the quanty library: {topic}{kept_to}\n\
         \n\
         1. Call `catalog` to see what the server offers, then use {find}.\n\
         2. Call `document_overview` for the most promising documents: their insights, takeaways and concepts say what each is about. With `detail` set to \"full\" it also gives the outline.\n\
         3. Call `read_section` for the sections of those documents that bear on the topic, with the `index` of each section from the outline.\n\
         4. Call `concept_neighbours` on the key concepts to find what they rest on and what rests on them, and follow what looks relevant.\n\
         5. Call `read_item` for the formulas, figures, tables and code that the answer needs.\n\
         \n\
         Write the answer with the document and page of every claim cited. Say plainly what the library does not cover."
    )
}

fn summarise_document(document_id: &str) -> String {
    format!(
        "Summarise the quanty library document {document_id}.\n\
         \n\
         1. Call `document_overview` with `document_id` set to {document_id:?} and `detail` set to \"full\": the insights, takeaways, cheat sheet and concepts.\n\
         2. Call `document_outline` to see its sections.\n\
         3. Call `read_section` for the sections that matter most to the summary.\n\
         \n\
         Write the summary with the pages it rests on cited."
    )
}

fn ingest_book(media: &str, folder: &str) -> String {
    format!(
        "Add the book {media:?} to the quanty library from the folder {folder}, which is on the machine the server runs on. \
         Its chapters are the files named `chapter-<number>-<name>.pdf`. Take them one at a time, in the order of their numbers, and start no chapter before the job of the one before it has ended.\n\
         \n\
         For each chapter:\n\
         1. Call `check_pdf` with `media` set to {media:?} and the `path` of the chapter. It is free.\n\
         2. Tell the person the pages and the cost estimate, and wait for a yes before anything is paid for.\n\
         3. Call `ingest_pdf` with the same `media` and `path`. It is paid.\n\
         4. Call `watch_job` with the `job_id` it gave, again and again, until the job ends.\n\
         \n\
         When the last chapter has ended, say which chapters were added and what they cost."
    )
}
