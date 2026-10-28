//! Checks the library read as resources and prompts. Over the committed sample chapters with the
//! stores down: the templates, reads that give what the matching tool gives, addresses that name
//! nothing, the prompts, and the cache hints that only a newer client is given. Over a stored
//! library: the list of resources, a media and a concept.

use rmcp::ServiceError;
use rmcp::model::{
    CacheScope, GetPromptRequestParams, ReadResourceRequestParams, ReadResourceResult,
    ResourceContents,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::support::{
    Client, ClosedPorts, Library, call, connect, connect_on_2026_07_28, sample_content_folder,
    sample_document_id, sample_notes_document_id, structured,
};

const TEMPLATES: [&str; 7] = [
    "quanty://media/{title}",
    "quanty://document/{id}",
    "quanty://document/{id}/outline",
    "quanty://document/{id}/section/{n}",
    "quanty://document/{id}/page/{n}",
    "quanty://document/{id}/insights",
    "quanty://concept/{id}",
];

const FIVE_MINUTES_MS: u64 = 300_000;
const A_MINUTE_MS: u64 = 60_000;
const AN_HOUR_MS: u64 = 3_600_000;

/// The code of a protocol error, which a client gets as a failed request.
fn protocol_error_code(error: ServiceError) -> i32 {
    match error {
        ServiceError::McpError(error) => error.code.0,
        other => panic!("a protocol error was expected, not {other}"),
    }
}

fn text_of(read: &ReadResourceResult) -> (&str, &str, Value) {
    let [
        ResourceContents::TextResourceContents {
            uri,
            mime_type,
            text,
            ..
        },
    ] = read.contents.as_slice()
    else {
        panic!("a read gives one text: {:?}", read.contents);
    };
    let mime_type = mime_type.as_deref().expect("a read names its type");
    (
        uri,
        mime_type,
        serde_json::from_str(text).expect("a read gives JSON"),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn resources_and_prompts_answer_like_the_tools_with_cache_hints_only_for_new_clients() {
    let ports = ClosedPorts::new().with_content_folder(&sample_content_folder());

    the_templates_are_listed_with_hints_for_a_new_client(&ports).await;
    a_read_gives_what_the_matching_tool_gives_and_says_how_long_it_stays_fresh(&ports).await;
    an_address_that_names_nothing_is_a_missing_resource(&ports).await;
    the_prompts_name_the_tools_in_order_and_refuse_a_missing_argument(&ports).await;
    an_old_client_is_given_no_cache_hints(&ports).await;
}

async fn the_templates_are_listed_with_hints_for_a_new_client(ports: &ClosedPorts) {
    let client = connect_on_2026_07_28(ports.server()).await;

    let listed = client.list_resource_templates(None).await.unwrap();

    let uris: Vec<&str> = listed
        .resource_templates
        .iter()
        .map(|template| template.uri_template.as_str())
        .collect();
    assert_eq!(uris, TEMPLATES);
    assert!(
        listed
            .resource_templates
            .iter()
            .all(|template| template.mime_type.as_deref() == Some("application/json"))
    );
    assert_eq!(listed.ttl_ms, Some(FIVE_MINUTES_MS));
    assert_eq!(listed.cache_scope, Some(CacheScope::Public));
}

async fn a_read_gives_what_the_matching_tool_gives_and_says_how_long_it_stays_fresh(
    ports: &ClosedPorts,
) {
    let client = connect_on_2026_07_28(ports.server()).await;
    let id = sample_document_id().to_string();
    let notes = sample_notes_document_id(2).to_string();
    let outline = call(&client, "document_outline", json!({ "document_id": id })).await;
    let last_section = structured(&outline)["sections"].as_array().unwrap().len();
    let reads = [
        (
            format!("quanty://document/{id}"),
            "document_overview",
            json!({ "document_id": id }),
            A_MINUTE_MS,
        ),
        (
            format!("quanty://document/{id}/outline"),
            "document_outline",
            json!({ "document_id": id }),
            AN_HOUR_MS,
        ),
        (
            format!("quanty://document/{id}/section/{last_section}"),
            "read_section",
            json!({ "document_id": id, "section": last_section }),
            AN_HOUR_MS,
        ),
        (
            format!("quanty://document/{id}/page/5"),
            "read_page",
            json!({ "document_id": id, "page": 5 }),
            AN_HOUR_MS,
        ),
        (
            format!("quanty://document/{notes}/insights"),
            "chapter_insights",
            json!({ "document_id": notes }),
            A_MINUTE_MS,
        ),
    ];

    for (uri, tool, arguments, fresh_for) in reads {
        let read = client
            .read_resource(ReadResourceRequestParams::new(uri.clone()))
            .await
            .unwrap_or_else(|error| panic!("{uri} should be read: {error}"));

        let (read_uri, mime_type, view) = text_of(&read);
        assert_eq!(read_uri, uri);
        assert_eq!(mime_type, "application/json");
        let called = call(&client, tool, arguments).await;
        assert_eq!(&view, structured(&called), "{uri} gives what {tool} gives");
        assert_eq!(read.ttl_ms, Some(fresh_for), "{uri}");
        assert_eq!(read.cache_scope, Some(CacheScope::Private), "{uri}");
    }
}

async fn an_address_that_names_nothing_is_a_missing_resource(ports: &ClosedPorts) {
    let id = sample_document_id();
    let unknown_document = Uuid::new_v4();
    let nothing = [
        "quanty://nothing/here".to_owned(),
        "other://document/anything".to_owned(),
        format!("quanty://document/{id}/section/x"),
        format!("quanty://document/{id}/page/999"),
        format!("quanty://document/{unknown_document}/outline"),
    ];
    // The client that negotiated the older protocol gets the code of a missing resource, and a
    // client of the newer protocol gets the code of invalid params for the same error.
    let older = connect(ports.server()).await;
    let newer = connect_on_2026_07_28(ports.server()).await;

    for uri in nothing {
        let params = || ReadResourceRequestParams::new(uri.as_str());
        let older_code = protocol_error_code(older.read_resource(params()).await.unwrap_err());
        let newer_code = protocol_error_code(newer.read_resource(params()).await.unwrap_err());
        assert_eq!(older_code, -32002, "{uri}");
        assert_eq!(newer_code, -32602, "{uri}");
    }

    let ServiceError::McpError(error) = older
        .read_resource(ReadResourceRequestParams::new("quanty://nothing/here"))
        .await
        .unwrap_err()
    else {
        panic!("a protocol error was expected");
    };
    assert_eq!(error.data, Some(json!({ "uri": "quanty://nothing/here" })));
    let ServiceError::McpError(error) = older
        .read_resource(ReadResourceRequestParams::new(format!(
            "quanty://document/{id}/page/999"
        )))
        .await
        .unwrap_err()
    else {
        panic!("a protocol error was expected");
    };
    assert!(
        error.message.contains("page 999 is out of range"),
        "{}",
        error.message
    );
}

async fn the_prompts_name_the_tools_in_order_and_refuse_a_missing_argument(ports: &ClosedPorts) {
    let client = connect_on_2026_07_28(ports.server()).await;

    let listed = client.list_prompts(None).await.unwrap();

    let prompts: Vec<Value> = listed
        .prompts
        .iter()
        .map(|prompt| {
            let arguments = prompt.arguments.iter().flatten();
            let arguments: Vec<Value> = arguments
                .map(|a| json!({ "name": a.name, "required": a.required }))
                .collect();
            json!({ "name": prompt.name, "arguments": arguments })
        })
        .collect();
    assert_eq!(
        prompts,
        [
            json!({ "name": "research_topic", "arguments": [
                { "name": "topic", "required": true },
                { "name": "media", "required": false },
            ]}),
            json!({ "name": "summarise_document", "arguments": [
                { "name": "document_id", "required": true },
            ]}),
            json!({ "name": "ingest_book", "arguments": [
                { "name": "media", "required": true },
                { "name": "folder", "required": true },
            ]}),
        ]
    );
    assert_eq!(listed.ttl_ms, Some(FIVE_MINUTES_MS));
    assert_eq!(listed.cache_scope, Some(CacheScope::Public));

    let arguments = json!({ "topic": "gamma scalping", "media": "Option Volatility and Pricing" });
    let got = client
        .get_prompt(
            GetPromptRequestParams::new("research_topic")
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap();

    let [message] = got.messages.as_slice() else {
        panic!("one message was expected: {:?}", got.messages);
    };
    let text = message
        .content
        .as_text()
        .expect("a prompt is text")
        .text
        .as_str();
    assert!(text.contains("gamma scalping"), "{text}");
    assert!(text.contains("Option Volatility and Pricing"), "{text}");
    let places: Vec<usize> = [
        "`catalog`",
        "`document_overview`",
        "`read_section`",
        "`concept_neighbours`",
        "`read_item`",
    ]
    .map(|tool| {
        text.find(tool)
            .unwrap_or_else(|| panic!("{tool} is not in {text}"))
    })
    .to_vec();
    assert!(
        places.is_sorted(),
        "the tools are named in order: {places:?}"
    );

    let missing = client
        .get_prompt(GetPromptRequestParams::new("research_topic"))
        .await
        .unwrap_err();
    assert_eq!(protocol_error_code(missing), -32602);
    let unknown = client
        .get_prompt(GetPromptRequestParams::new("no_such_prompt"))
        .await
        .unwrap_err();
    assert_eq!(protocol_error_code(unknown), -32602);
}

async fn an_old_client_is_given_no_cache_hints(ports: &ClosedPorts) {
    let client = connect(ports.server()).await;
    let id = sample_document_id();

    let templates = client.list_resource_templates(None).await.unwrap();
    let read = client
        .read_resource(ReadResourceRequestParams::new(format!(
            "quanty://document/{id}/outline"
        )))
        .await
        .unwrap();
    let prompts = client.list_prompts(None).await.unwrap();

    assert_eq!(templates.resource_templates.len(), TEMPLATES.len());
    assert_eq!((templates.ttl_ms, templates.cache_scope), (None, None));
    assert_eq!((read.ttl_ms, read.cache_scope), (None, None));
    assert_eq!(prompts.prompts.len(), 3);
    assert_eq!((prompts.ttl_ms, prompts.cache_scope), (None, None));
}

/// The JSON a read of `uri` gives, with how long it stays fresh and for whom.
async fn read_json(client: &Client, uri: &str) -> (Value, Option<u64>, Option<CacheScope>) {
    let read = client
        .read_resource(ReadResourceRequestParams::new(uri))
        .await
        .unwrap_or_else(|error| panic!("{uri} should be read: {error}"));
    let (_, _, view) = text_of(&read);
    (view, read.ttl_ms, read.cache_scope)
}

/// Takes a library that a store test has seeded, so that the stores are seeded once for all the
/// cases of that test.
pub(crate) async fn the_library_is_listed_as_resources_and_a_media_and_a_concept_are_read(
    library: &Library,
) {
    let client = connect_on_2026_07_28(library.server()).await;
    let names = call(&client, "list_documents", json!({ "detail": "names" })).await;
    let documents: Vec<(String, String)> = structured(&names)["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|document| {
            (
                document["document_id"].as_str().unwrap().to_owned(),
                document["title"].as_str().unwrap().to_owned(),
            )
        })
        .collect();

    let listed = client.list_resources(None).await.unwrap();

    let mut expected = vec![
        (
            "quanty://media/Option%20Volatility%20and%20Pricing".to_owned(),
            "Option Volatility and Pricing".to_owned(),
        ),
        (
            "quanty://media/Quanty%20Sample%20Notes".to_owned(),
            "Quanty Sample Notes".to_owned(),
        ),
    ];
    expected.extend(
        documents
            .iter()
            .map(|(id, title)| (format!("quanty://document/{id}"), title.clone())),
    );
    let found: Vec<(String, String)> = listed
        .resources
        .iter()
        .map(|resource| (resource.uri.clone(), resource.name.clone()))
        .collect();
    assert_eq!(found, expected);
    assert_eq!(documents.len(), 3);
    assert_eq!(listed.ttl_ms, Some(60_000));
    assert_eq!(listed.cache_scope, Some(CacheScope::Private));

    let overview = call(
        &client,
        "document_overview",
        json!({ "document_id": sample_document_id().to_string() }),
    )
    .await;
    let concept_id = structured(&overview)["concepts"][0]["concept_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let concept = call(&client, "concept", json!({ "concept_id": concept_id })).await;

    let (read, ttl_ms, scope) = read_json(&client, &format!("quanty://concept/{concept_id}")).await;

    assert_eq!(&read, structured(&concept));
    assert_eq!(read["name"], "Long straddle");
    assert_eq!((ttl_ms, scope), (Some(60_000), Some(CacheScope::Private)));

    let notes = call(
        &client,
        "list_documents",
        json!({ "media": "Quanty Sample Notes" }),
    )
    .await;

    let (read, ttl_ms, scope) = read_json(&client, "quanty://media/Quanty%20Sample%20Notes").await;

    assert_eq!(&read, structured(&notes));
    assert_eq!(read["total"], 2);
    assert_eq!((ttl_ms, scope), (Some(60_000), Some(CacheScope::Private)));

    let nothing = client
        .read_resource(ReadResourceRequestParams::new(
            "quanty://media/No%20Such%20Book",
        ))
        .await
        .unwrap_err();
    let ServiceError::McpError(error) = nothing else {
        panic!("a protocol error was expected");
    };
    assert!(error.message.contains("No Such Book"), "{}", error.message);
}
