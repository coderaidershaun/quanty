//! Checks the tasks extension: a client that declares it follows each job as a task, can cancel
//! the job through its task, and gets an error for an id that is no job. A job is held at its page
//! gate, so each test sees the task in the state it chose.

use std::time::{Duration, Instant};

use mcp::{BackgroundJobs, QuantyServer, http_service};
use ocr::testing::sample_pdf;
use rag_ingestion::testing::ThrowawayStores;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CancelTaskParams, CreateTaskResult, ErrorCode,
    GetTaskParams, InputResponses, JsonObject, ProtocolVersion, TaskPayload, TaskStatus,
    UpdateTaskParams,
};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{ClientLifecycleMode, ClientServiceExt, RoleClient, ServiceError};
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::{
    ClosedPorts, STAGE_WAIT, StandInServices, call, connect_declaring_tasks, connect_on_2026_07_28,
    status_in_stage, structured, tasks_client_config,
};

const MEDIA: &str = "Option Volatility and Pricing";

/// How long a test waits for a task to end: an ingest of seven pages of stand-in calls.
const TASK_WAIT: Duration = Duration::from_secs(180);

/// Starts a tool that starts a job, as a client that declared tasks, and gives the task it got.
async fn start_task(
    client: &RunningService<RoleClient, impl rmcp::service::Service<RoleClient>>,
    tool: &'static str,
    arguments: Value,
) -> CreateTaskResult {
    let Value::Object(arguments) = arguments else {
        panic!("the arguments of a tool call are a JSON object, not {arguments}");
    };
    let response = client
        .call_tool_once(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await
        .unwrap_or_else(|error| panic!("the call of {tool} should get a response: {error}"));
    let CallToolResponse::Task(created) = response else {
        panic!("{tool} should answer a client that declared tasks with a task: {response:?}");
    };
    created
}

/// Polls `tasks/get` until the task is `completed`, and gives the tool result it carries.
async fn result_when_completed(
    client: &RunningService<RoleClient, impl rmcp::service::Service<RoleClient>>,
    task_id: &str,
) -> JsonObject {
    let started = Instant::now();
    loop {
        let task = client
            .get_task(GetTaskParams::new(task_id))
            .await
            .expect("tasks/get should answer");
        match task.task.payload {
            TaskPayload::Completed { result } => return result,
            TaskPayload::Working => {}
            other => panic!("the task should work and then complete, not {other:?}"),
        }
        assert!(
            started.elapsed() < TASK_WAIT,
            "the task did not complete in time"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// The task `created` is the one of the job `job_status` finds, and ends with its result.
async fn assert_task_follows_job(
    client: &RunningService<RoleClient, impl rmcp::service::Service<RoleClient>>,
    created: &CreateTaskResult,
    kind: &str,
) {
    let wire = serde_json::to_value(created).unwrap();
    assert_eq!(wire["resultType"], "task", "{wire}");
    assert_eq!(wire["pollIntervalMs"], 5000, "{wire}");
    assert_eq!(wire["ttlMs"], 86_400_000, "{wire}");
    let job_id = created.task.task_id.as_str();
    let job = structured(&call(client, "job_status", json!({ "job_id": job_id })).await).clone();
    assert_eq!(job["kind"], kind, "the task id is the job id: {job}");

    let result = result_when_completed(client, job_id).await;

    assert_eq!(result["isError"], false, "{result:?}");
    let status = call(client, "job_status", json!({ "job_id": job_id })).await;
    assert_eq!(result["structuredContent"], *structured(&status));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored jobs::tasks::"]
async fn a_client_that_declares_tasks_follows_each_start_tool_as_a_task() {
    let (services, open_the_pages) = StandInServices::new().with_page_gate();
    let stores = ThrowawayStores::new("mcp-tasks-follow");
    let server = QuantyServer::new(stores.config().clone(), services);
    let client = connect_declaring_tasks(server.clone()).await;

    let created = start_task(
        &client,
        "ingest_pdf",
        json!({ "media": MEDIA, "path": sample_pdf() }),
    )
    .await;

    let job_id = created.task.task_id.clone();
    assert_eq!(created.task.status, TaskStatus::Working);
    let held = status_in_stage(&client, &job_id, "converting").await;
    let words = held["stage"]["words"].as_str().unwrap();
    assert_eq!(words, "converting the pages: 0 of 7 done");
    let working = client.get_task(GetTaskParams::new(&job_id)).await.unwrap();
    assert_eq!(working.task.payload, TaskPayload::Working);
    assert_eq!(working.task.task.status_message.as_deref(), Some(words));

    drop(open_the_pages);
    assert_task_follows_job(&client, &created, "ingest").await;

    let insights = start_task(
        &client,
        "write_insights",
        json!({ "all": true, "dry_run": true }),
    )
    .await;
    assert_task_follows_job(&client, &insights, "insights").await;

    let dataset = start_task(
        &client,
        "build_dataset",
        json!({ "name": "followed", "dry_run": true }),
    )
    .await;
    assert_task_follows_job(&client, &dataset, "dataset").await;

    // The PDF is stored now. A client that did not declare tasks gets the report, not a task.
    let plain = connect_on_2026_07_28(server).await;
    let again = call(
        &plain,
        "ingest_pdf",
        json!({ "media": MEDIA, "path": sample_pdf() }),
    )
    .await;
    assert_eq!(structured(&again)["state"], "already_ingested");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p mcp --test integration -- --ignored jobs::tasks::"]
async fn cancelling_a_task_stops_its_job() {
    let slot = BackgroundJobs::new();
    let (services, open_the_pages) = StandInServices::new().with_page_gate();
    let stores = ThrowawayStores::new("mcp-tasks-cancel");
    let server =
        QuantyServer::with_background_jobs(stores.config().clone(), services, slot.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = http_service(server);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let transport = StreamableHttpClientTransport::from_uri(format!("http://{address}/mcp"));
    let client = tasks_client_config()
        .serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        )
        .await
        .expect("the client should connect over streamable HTTP");
    let created = start_task(
        &client,
        "ingest_pdf",
        json!({ "media": MEDIA, "path": sample_pdf() }),
    )
    .await;
    let job_id = created.task.task_id;
    status_in_stage(&client, &job_id, "converting").await;
    assert!(slot.running().is_some(), "the job holds the shared slot");

    client
        .cancel_task(CancelTaskParams::new(&job_id))
        .await
        .expect("tasks/cancel should be acknowledged");

    let cancelled = client.get_task(GetTaskParams::new(&job_id)).await.unwrap();
    assert_eq!(cancelled.task.payload, TaskPayload::Cancelled);
    let message = cancelled.task.task.status_message.expect("what to do");
    assert!(message.contains("stopped"), "{message}");
    let job = call(&client, "job_status", json!({ "job_id": job_id })).await;
    assert_eq!(structured(&job)["state"], "stopped");
    let released = async {
        while slot.running().is_some() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::time::timeout(STAGE_WAIT, released)
        .await
        .expect("the slot should be free once the job is stopped");
    drop(open_the_pages);
}

/// Asserts that a call was refused with the error for a parameter that is wrong.
fn assert_unknown_job_error<T: std::fmt::Debug>(refused: Result<T, ServiceError>) {
    let Err(ServiceError::McpError(error)) = refused else {
        panic!("the call should be refused with a protocol error, not {refused:?}");
    };
    assert_eq!(error.code, ErrorCode::INVALID_PARAMS, "{error:?}");
    assert!(
        error.message.contains("no job of this server has the id"),
        "{error:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_task_id_is_an_invalid_params_error() {
    let ports = ClosedPorts::new();
    let client = connect_declaring_tasks(ports.server()).await;

    assert_unknown_job_error(client.get_task(GetTaskParams::new("no-such-job")).await);
    assert_unknown_job_error(
        client
            .cancel_task(CancelTaskParams::new("no-such-job"))
            .await,
    );
    assert_unknown_job_error(
        client
            .update_task(UpdateTaskParams::new("no-such-job", InputResponses::new()))
            .await,
    );
}
