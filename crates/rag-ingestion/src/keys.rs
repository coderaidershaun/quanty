//! What the two API keys of the settings can do, found with the one free call each provider
//! allows. Neither provider gives a balance to a key, so the answer is a state and never an amount.

use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;

use ocr::convert::services::{Jev, JevError};
use rag_core::{ApiKey, Config, EmbedError, GeminiEmbedder};

/// A provider that neither answers nor refuses must not hold a caller for ever.
pub const KEY_CHECK_LIMIT: Duration = Duration::from_secs(5);

/// What one provider said about the key of the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyState {
    /// The settings hold no key, so nothing was sent.
    Missing,
    /// The provider does not accept the key.
    Refused,
    /// The provider says the prepaid credit is used up.
    OutOfCredit,
    /// The provider accepted the key. It does not say how much credit is left.
    Accepted,
    /// The check could not tell.
    Unread(KeyReadFailure),
}

/// Why a check could not tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyReadFailure {
    /// The provider could not be reached.
    Unreachable,
    /// The provider gave no answer within [`KEY_CHECK_LIMIT`].
    TimedOut,
    /// The provider answered with this HTTP status, which is none that says the key is refused.
    Status(u16),
    /// The check failed inside quanty.
    Internal,
}

/// The state of each key of the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyStates {
    pub gemini: KeyState,
    pub jev: KeyState,
}

impl KeyState {
    /// The state that the answer of Google's free read of the embedding model means. A key that is
    /// out of credit is told before one that is refused.
    pub fn of_gemini(checked: &Result<(), EmbedError>) -> KeyState {
        match checked {
            Ok(()) => KeyState::Accepted,
            Err(error) if error.is_out_of_credit() => KeyState::OutOfCredit,
            Err(error) if error.is_key_refused() => KeyState::Refused,
            Err(EmbedError::Rejected { status, .. }) => {
                KeyState::Unread(KeyReadFailure::Status(*status))
            }
            Err(EmbedError::Transport(_)) => KeyState::Unread(KeyReadFailure::Unreachable),
            Err(_) => KeyState::Unread(KeyReadFailure::Internal),
        }
    }

    /// The state that the answer of Jev's free list of models means. A key that is out of credit
    /// is told before one that is refused.
    pub fn of_jev(checked: &Result<(), JevError>) -> KeyState {
        match checked {
            Ok(()) => KeyState::Accepted,
            Err(error) if error.is_out_of_credit() => KeyState::OutOfCredit,
            Err(error) if error.is_key_refused() => KeyState::Refused,
            Err(JevError::Rejected { status, .. }) => {
                KeyState::Unread(KeyReadFailure::Status(*status))
            }
            Err(JevError::Http(_)) => KeyState::Unread(KeyReadFailure::Unreachable),
            Err(_) => KeyState::Unread(KeyReadFailure::Internal),
        }
    }
}

/// Waits for both reads at once, each for at most [`KEY_CHECK_LIMIT`]. `None` is a key that the
/// settings do not hold, and nothing is sent for it. The limit covers the network read only: the
/// client must be made before, because building one can block for seconds on some machines.
pub async fn read_key_states(
    gemini: Option<impl Future<Output = Result<(), EmbedError>>>,
    jev: Option<impl Future<Output = Result<(), JevError>>>,
) -> KeyStates {
    let (gemini, jev) = tokio::join!(
        within_limit(gemini, KeyState::of_gemini),
        within_limit(jev, KeyState::of_jev),
    );
    KeyStates { gemini, jev }
}

async fn within_limit<E>(
    check: Option<impl Future<Output = Result<(), E>>>,
    state_of: fn(&Result<(), E>) -> KeyState,
) -> KeyState {
    let Some(check) = check else {
        return KeyState::Missing;
    };
    match tokio::time::timeout(KEY_CHECK_LIMIT, check).await {
        Ok(checked) => state_of(&checked),
        Err(_) => KeyState::Unread(KeyReadFailure::TimedOut),
    }
}

/// The clients that ask Google and Jev about a key, each made on first use and then kept for the
/// whole run of the program.
///
/// Warning: a kept client holds the key of the call that made it, and a later call with another
/// key still asks about the first one. A program whose settings can change needs a new value.
#[derive(Default)]
pub struct KeyClients {
    gemini: OnceLock<GeminiEmbedder>,
    jev: OnceLock<Jev>,
}

impl KeyClients {
    /// The client for Google, made here the first time so that a time limit never covers it. A
    /// client that cannot be made is not kept. Two calls at the same moment may each make one, and
    /// the first to finish is the one that stays.
    ///
    /// # Errors
    /// [`EmbedError::Transport`] when the HTTP client cannot be built.
    pub fn gemini(&self, key: &ApiKey) -> Result<&GeminiEmbedder, EmbedError> {
        if let Some(client) = self.gemini.get() {
            return Ok(client);
        }
        let client = GeminiEmbedder::new(key)?;
        Ok(self.gemini.get_or_init(|| client))
    }

    /// The client for Jev, made here the first time like [`KeyClients::gemini`].
    ///
    /// # Errors
    /// [`JevError::Http`] when the HTTP client cannot be built.
    pub fn jev(&self, key: &ApiKey) -> Result<&Jev, JevError> {
        if let Some(client) = self.jev.get() {
            return Ok(client);
        }
        let client = Jev::new(key.expose())?;
        Ok(self.jev.get_or_init(|| client))
    }

    /// Asks both providers about the keys of `config` at once, with the one free call each allows.
    /// The clients are made when this is called, and the future that it returns is only the two
    /// timed reads. It never fails: a check that cannot tell is a state.
    pub fn check(&self, config: &Config) -> impl Future<Output = KeyStates> + Send {
        let gemini = config.gemini_api_key.as_ref().map(|key| self.gemini(key));
        let jev = config.jev_api_key.as_ref().map(|key| self.jev(key));
        read_key_states(
            gemini.map(|client| async move { client?.check_key().await }),
            jev.map(|client| async move { client?.check_key().await }),
        )
    }
}
