//! The result of the `health` tool: whether the services that a tool needs are ready, how each
//! of the two API keys stands, and the model that each kind of work uses.

use rag_core::{ModelSettings, SettingsError, WorkKind, error_chain};
use rag_ingestion::health::HealthReport;
use rag_ingestion::{KEY_CHECK_LIMIT, KeyReadFailure, KeyState, KeyStates};
use schemars::JsonSchema;
use serde::Serialize;

/// Where the credit of the Gemini key is seen and added. Google gives no balance to a key.
pub(super) const GEMINI_BILLING_PAGE: &str = "https://aistudio.google.com/billing";

/// Where the credit of the Jev key is seen and added. Jev gives no balance to a key.
pub(super) const JEV_CONSOLE_PAGE: &str = "https://console.typesafe.ai/";

/// Whether the services that a tool needs are ready.
#[derive(Serialize, JsonSchema)]
pub(super) struct HealthResult {
    /// True when Qdrant, FalkorDB and the sign-in of the Ingest choice's agent are all ready and
    /// both API keys are accepted. A key that could not be checked counts as not ready.
    healthy: bool,
    /// One line for each service and each key, saying whether it is ready.
    report: String,
    /// The Google Gemini key, then the Jev key.
    keys: Vec<KeyView>,
    /// The model and thinking that each kind of work uses, as saved. This server cannot change
    /// them.
    models: Vec<ModelView>,
}

/// How one API key stands.
#[derive(Serialize, JsonSchema)]
struct KeyView {
    /// How the line of the report names the key.
    #[serde(skip)]
    name_in_report: &'static str,
    /// The setting that holds the key.
    key: &'static str,
    /// The provider the key belongs to.
    service: &'static str,
    state: KeyStateName,
    /// What the state means and what to do about it.
    detail: String,
    /// Where the credit is seen and added: neither provider gives a balance to a key.
    billing_page: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum KeyStateName {
    /// The provider accepted the key. It does not say how much credit is left.
    Accepted,
    /// The settings hold no key.
    Missing,
    /// The provider does not accept the key.
    Refused,
    /// The provider says the prepaid credit is used up.
    OutOfCredit,
    /// The check could not tell. `detail` says why.
    Unread,
}

/// The model choice of one kind of work.
#[derive(Serialize, JsonSchema)]
struct ModelView {
    /// `ask`, `ingest` or `datasets`.
    work: &'static str,
    /// The tools that spend this choice.
    tools: &'static [&'static str],
    /// `claude` or `codex`. Left out when the saved choice cannot be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<String>,
    /// Why the saved choice cannot be read. Only then.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// What differs between the two providers in the words about a key.
struct Provider {
    key: &'static str,
    service: &'static str,
    /// The name of the key in the report.
    in_report: &'static str,
    /// The company that gives no balance to a key.
    company: &'static str,
    /// What the page of the credit is called.
    account_page: &'static str,
    billing_page: &'static str,
}

const GEMINI: Provider = Provider {
    key: "EMBEDDING_GEMINI_API_KEY",
    service: "Google Gemini",
    in_report: "Gemini key",
    company: "Google",
    account_page: "billing page",
    billing_page: GEMINI_BILLING_PAGE,
};

const JEV: Provider = Provider {
    key: "CONVERTER_JEV_API_KEY",
    service: "Jev",
    in_report: "Jev key",
    company: "Jev",
    account_page: "console",
    billing_page: JEV_CONSOLE_PAGE,
};

impl HealthResult {
    /// The result for a report of the stores and the agent, the states of the two keys and the
    /// model choice of each kind of work.
    pub(super) fn new(
        services: &HealthReport,
        keys: KeyStates,
        model_choice: impl Fn(WorkKind) -> Result<ModelSettings, SettingsError>,
    ) -> HealthResult {
        let keys = [
            KeyView::of(&GEMINI, keys.gemini),
            KeyView::of(&JEV, keys.jev),
        ];
        let healthy =
            services.is_healthy() && keys.iter().all(|key| key.state == KeyStateName::Accepted);
        let mut lines = vec![services.to_string()];
        lines.extend(keys.iter().map(KeyView::line));
        HealthResult {
            healthy,
            report: lines.join("\n"),
            keys: keys.into(),
            models: WorkKind::ALL
                .into_iter()
                .map(|work| ModelView::of(work, model_choice(work)))
                .collect(),
        }
    }
}

impl KeyView {
    fn of(provider: &Provider, state: KeyState) -> KeyView {
        let (state, detail) = match state {
            KeyState::Accepted => (
                KeyStateName::Accepted,
                format!(
                    "the key was accepted; {} gives no balance to a key, so the credit is on the {}",
                    provider.company, provider.account_page
                ),
            ),
            KeyState::Missing => (
                KeyStateName::Missing,
                format!(
                    "{} is not set; add it to .env or to the environment, then start the server again",
                    provider.key
                ),
            ),
            KeyState::Refused => (
                KeyStateName::Refused,
                format!(
                    "the provider refused the key; check it on the {}, put a good key in .env, then start the server again",
                    provider.account_page
                ),
            ),
            KeyState::OutOfCredit => (
                KeyStateName::OutOfCredit,
                format!(
                    "the provider says the prepaid credit is used up; add credit on the {}",
                    provider.account_page
                ),
            ),
            KeyState::Unread(failure) => (KeyStateName::Unread, unread_detail(failure)),
        };
        KeyView {
            name_in_report: provider.in_report,
            key: provider.key,
            service: provider.service,
            state,
            detail,
            billing_page: provider.billing_page,
        }
    }

    /// The line of the report: `ok` for a key that is accepted, `FAILED` for any other.
    fn line(&self) -> String {
        let name = self.name_in_report;
        match self.state {
            KeyStateName::Accepted => format!("{name}: ok (accepted)"),
            state => format!("{name}: FAILED ({}): {}", state.as_str(), self.detail),
        }
    }
}

impl KeyStateName {
    fn as_str(self) -> &'static str {
        match self {
            KeyStateName::Accepted => "accepted",
            KeyStateName::Missing => "missing",
            KeyStateName::Refused => "refused",
            KeyStateName::OutOfCredit => "out_of_credit",
            KeyStateName::Unread => "unread",
        }
    }
}

fn unread_detail(failure: KeyReadFailure) -> String {
    match failure {
        KeyReadFailure::Unreachable => {
            "the provider could not be reached; check the network".to_owned()
        }
        KeyReadFailure::TimedOut => format!(
            "the provider gave no answer within {} seconds",
            KEY_CHECK_LIMIT.as_secs()
        ),
        KeyReadFailure::Status(status) => format!("the provider answered with HTTP {status}"),
        KeyReadFailure::Internal => "the check failed inside quanty".to_owned(),
    }
}

impl ModelView {
    fn of(work: WorkKind, choice: Result<ModelSettings, SettingsError>) -> ModelView {
        let (name, tools): (&str, &'static [&'static str]) = match work {
            WorkKind::Ask => ("ask", &["search", "answer"]),
            WorkKind::Ingest => ("ingest", &["ingest_pdf", "write_insights"]),
            WorkKind::Datasets => ("datasets", &["build_dataset"]),
        };
        let view = ModelView {
            work: name,
            tools,
            provider: None,
            model: None,
            thinking: None,
            error: None,
        };
        match choice {
            Ok(choice) => ModelView {
                provider: Some(choice.provider),
                model: Some(choice.model),
                thinking: Some(choice.thinking),
                ..view
            },
            Err(error) => ModelView {
                error: Some(error_chain(&error)),
                ..view
            },
        }
    }
}
