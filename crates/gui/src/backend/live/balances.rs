//! Reads what each paid account shows, by asking each provider whether it accepts the key. No
//! provider gives a balance, so the answer is never an amount.

use rag_ingestion::{KeyReadFailure, KeyState, read_key_states};

use super::context::{LiveContext, Services};
use crate::backend::Reply;
use crate::contract::{Balance, BalanceReads, Event, ReadFailure, RequestId};

/// Both providers are asked at once, and the one answer is sent when the slower has answered or
/// has run out of time. The clients are made before the time limit starts.
pub(super) async fn read<S: Services>(cx: &LiveContext<S>, request: RequestId, reply: &Reply) {
    let states = read_key_states(cx.check_gemini_key(), cx.check_jev_key()).await;
    let found = BalanceReads {
        google: balance(states.gemini),
        jev: balance(states.jev),
    };
    reply.send(Event::Balances { request, found });
}

fn balance(state: KeyState) -> Balance {
    match state {
        KeyState::Missing => Balance::KeyMissing,
        KeyState::Refused => Balance::KeyRefused,
        KeyState::OutOfCredit => Balance::OutOfCredit,
        KeyState::Accepted => Balance::NotOffered,
        KeyState::Unread(failure) => Balance::Unread(read_failure(failure)),
    }
}

fn read_failure(failure: KeyReadFailure) -> ReadFailure {
    match failure {
        KeyReadFailure::Unreachable => ReadFailure::Unreachable,
        KeyReadFailure::TimedOut => ReadFailure::TimedOut,
        KeyReadFailure::Status(status) => ReadFailure::Status(status),
        KeyReadFailure::Internal => ReadFailure::Internal,
    }
}
