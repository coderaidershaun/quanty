//! Free checks tell the browser which services can answer before any paid work starts.

use std::time::Duration;

use contract::{Event, Failure, FailureKind, Provider, RequestId, Service, ServiceState};
use rag_core::ItemStore;
use rag_ingestion::{KeyState, read_key_states};

use super::{LiveContext, Services};
use crate::Reply;

const CHECK_LIMIT: Duration = Duration::from_secs(5);

pub(super) async fn check<S: Services>(cx: &LiveContext<S>, request: RequestId, reply: &Reply) {
    let config = cx.config();
    let report = |service, state| {
        reply.send(Event::Health {
            request,
            service,
            state,
        })
    };
    let qdrant = async {
        let result = async {
            ItemStore::connect(&config)
                .map_err(|error| cx.failure(error))?
                .ping()
                .await
                .map_err(|error| cx.failure(error))
        };
        let state = bounded(result, Failure::qdrant_down(&config.qdrant_url)).await;
        report(
            Service::Qdrant,
            up_or_down(state, "the store answered its ping"),
        );
    };
    let graph = async {
        let result = async {
            cx.graph()
                .await?
                .ping()
                .await
                .map_err(|error| cx.failure(error))
        };
        let state = bounded(result, Failure::falkordb_down(&config.falkordb_url)).await;
        report(
            Service::FalkorDb,
            up_or_down(state, "the graph answered its ping"),
        );
    };
    let agent = async {
        let choice = choices::WorkChoices::load_from(&cx.home().join(choices::MODEL_SETTINGS_FILE));
        let state = match choice {
            Ok(choices) => {
                let detail = format!(
                    "Ingest uses {} model {}; its sign-in is ready",
                    choices.ingest.provider, choices.ingest.model
                );
                let failure = Failure::new(
                    FailureKind::TimedOut,
                    "the Ingest agent readiness check did not finish within five seconds",
                );
                up_or_down(bounded(cx.ingest_agent_ready(), failure).await, &detail)
            }
            Err(error) => ServiceState::Down(Failure::internal(format!(
                "could not read the Ingest model choice: {error}"
            ))),
        };
        report(Service::Claude, state);
    };
    let keys = async {
        let states = read_key_states(cx.check_gemini_key(), cx.check_jev_key()).await;
        report(
            Service::EmbeddingKey,
            key_state(states.gemini, Provider::Google),
        );
        report(Service::ConverterKey, key_state(states.jev, Provider::Jev));
    };
    let poppler = async {
        let check = async {
            ocr::check_poppler()
                .await
                .map_err(|error| Failure::new(FailureKind::PopplerMissing, error.to_string()))
        };
        let timeout = Failure::new(
            FailureKind::PopplerMissing,
            "the Poppler readiness check did not finish within five seconds",
        );
        let state = bounded(check, timeout).await;
        report(
            Service::Poppler,
            up_or_down(
                state,
                "pdfinfo, pdfseparate, pdftotext and pdftoppm report their versions",
            ),
        );
    };
    tokio::join!(qdrant, graph, agent, keys, poppler);
}

async fn bounded(
    check: impl Future<Output = Result<(), Failure>>,
    timeout: Failure,
) -> Result<(), Failure> {
    tokio::time::timeout(CHECK_LIMIT, check)
        .await
        .unwrap_or(Err(timeout))
}

fn up_or_down(result: Result<(), Failure>, detail: &str) -> ServiceState {
    match result {
        Ok(()) => ServiceState::Up {
            detail: detail.to_owned(),
        },
        Err(failure) => ServiceState::Down(failure),
    }
}

fn key_state(state: KeyState, provider: Provider) -> ServiceState {
    let (name, missing, credit) = match provider {
        Provider::Google => (
            "EMBEDDING_GEMINI_API_KEY",
            FailureKind::EmbeddingKeyMissing,
            FailureKind::EmbeddingOutOfCredit,
        ),
        Provider::Jev => (
            "CONVERTER_JEV_API_KEY",
            FailureKind::ConverterKeyMissing,
            FailureKind::ConverterOutOfCredit,
        ),
    };
    match state {
        KeyState::Accepted => ServiceState::Up {
            detail: "the provider accepted the key; available credit is not reported".into(),
        },
        KeyState::Unread(_) => ServiceState::Unknown,
        KeyState::Missing => {
            ServiceState::Down(Failure::new(missing, format!("{name} is not set")))
        }
        KeyState::OutOfCredit => ServiceState::Down(Failure::new(
            credit,
            "the provider reports that the account is out of credit",
        )),
        KeyState::Refused => ServiceState::Down(
            Failure::new(
                FailureKind::Internal,
                format!("the provider refused {name}"),
            )
            .with_hint(format!(
                "Replace {name} in the server settings, then check again."
            )),
        ),
    }
}
