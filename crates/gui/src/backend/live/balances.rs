//! Reads what each paid account shows, by asking each provider whether it accepts the key. No
//! provider gives a balance, so the answer is never an amount.

use std::time::Duration;

use ocr::convert::services::JevError;
use rag_core::EmbedError;
use tokio::time::error::Elapsed;

use super::context::{LiveContext, Services};
use crate::backend::Reply;
use crate::contract::{Balance, BalanceReads, Event, ReadFailure, RequestId};

/// A provider that neither answers nor refuses must not hold the footer for ever.
const READ_LIMIT: Duration = Duration::from_secs(5);

/// Both providers are asked at once, and the one answer is sent when the slower has answered or
/// has run out of time.
pub(super) async fn read<S: Services>(cx: &LiveContext<S>, request: RequestId, reply: &Reply) {
    let (google, jev) = tokio::join!(
        tokio::time::timeout(READ_LIMIT, cx.check_gemini_key()),
        tokio::time::timeout(READ_LIMIT, cx.check_jev_key()),
    );
    let found = BalanceReads {
        google: google_balance(google),
        jev: jev_balance(jev),
    };
    reply.send(Event::Balances { request, found });
}

fn google_balance(checked: Result<Option<Result<(), EmbedError>>, Elapsed>) -> Balance {
    match checked {
        Err(_) => Balance::Unread(ReadFailure::TimedOut),
        Ok(None) => Balance::KeyMissing,
        Ok(Some(Ok(()))) => Balance::NotOffered,
        Ok(Some(Err(error))) if error.is_out_of_credit() => Balance::OutOfCredit,
        Ok(Some(Err(error))) if error.is_key_refused() => Balance::KeyRefused,
        Ok(Some(Err(EmbedError::Rejected { status, .. }))) => {
            Balance::Unread(ReadFailure::Status(status))
        }
        Ok(Some(Err(EmbedError::Transport(_)))) => Balance::Unread(ReadFailure::Unreachable),
        Ok(Some(Err(_))) => Balance::Unread(ReadFailure::Internal),
    }
}

fn jev_balance(checked: Result<Option<Result<(), JevError>>, Elapsed>) -> Balance {
    match checked {
        Err(_) => Balance::Unread(ReadFailure::TimedOut),
        Ok(None) => Balance::KeyMissing,
        Ok(Some(Ok(()))) => Balance::NotOffered,
        Ok(Some(Err(error))) if error.is_out_of_credit() => Balance::OutOfCredit,
        Ok(Some(Err(error))) if error.is_key_refused() => Balance::KeyRefused,
        Ok(Some(Err(JevError::Rejected { status, .. }))) => {
            Balance::Unread(ReadFailure::Status(status))
        }
        Ok(Some(Err(JevError::Http(_)))) => Balance::Unread(ReadFailure::Unreachable),
        Ok(Some(Err(_))) => Balance::Unread(ReadFailure::Internal),
    }
}
