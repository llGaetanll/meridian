//! What a call to the server came to, when it did not come to an answer.

use client_state::LoadFailure;
use client_state::Unpaid;
use server_wire::UnpaidReason;
use server_wire::WireUnpaid;

/// Why a call did not come back with what was asked for.
///
/// No sentences. What the user reads is the machine's — every platform gets
/// one wording rather than three, and a sentence written here is one no
/// `eval!` test can read.
#[derive(Debug)]
pub enum Failure {
    /// Nothing came back at all.
    Unreachable,

    /// The server answered, and this is what it said.
    Answered(u16),

    /// **402**, and the body that came with it. The one refusal that carries
    /// anything: a status code cannot tell somebody who has never subscribed
    /// from somebody whose card stopped working, and those are two screens.
    Unpaid(Unpaid),

    /// An answer arrived and could not be read.
    Unreadable,

    /// Asked for something that needs a session, with no token in hand.
    NoSession,
}

impl Failure {
    /// The same fact, in the machine's vocabulary.
    ///
    /// Classification and not a verdict: which screen a 401 leads to is
    /// decided in `client-state`, and this only says which of these it was.
    pub fn failure(&self) -> LoadFailure {
        match self {
            Self::Unreachable => LoadFailure::Offline,
            Self::Unreadable => LoadFailure::Unreadable,
            Self::Unpaid(reason) => LoadFailure::Unpaid {
                reason: reason.clone(),
            },

            // Nothing to send means nothing the server would honour, which is
            // the same place a 401 leads.
            Self::NoSession => LoadFailure::SignedOut,

            Self::Answered(401) => LoadFailure::SignedOut,
            Self::Answered(429) => LoadFailure::TooMany,
            Self::Answered(status) => LoadFailure::Server { status: *status },
        }
    }

    /// What the server refused this with, or `None` if it did not.
    ///
    /// Every route's check in one place, so a refusal is read the same way
    /// whichever route met it — and so a 402's body is never the one route
    /// that forgot to look.
    pub fn refusal(status: u16, body: &str) -> Option<Self> {
        match status {
            200..300 => None,
            402 => Some(Self::Unpaid(unpaid(body))),
            status => Some(Self::Answered(status)),
        }
    }
}

/// Why a 402 was answered.
///
/// A body that does not parse reads as `Never`, which is the only one of the
/// four that claims nothing about what came before — so an old app meeting a
/// word it has never heard of says less rather than something untrue.
fn unpaid(body: &str) -> Unpaid {
    let Ok(said) = serde_json::from_str::<WireUnpaid>(body) else {
        return Unpaid::Never;
    };

    match said.reason() {
        UnpaidReason::Never => Unpaid::Never,
        UnpaidReason::Declined => Unpaid::Declined {
            ran_out: said.ran_out,
        },
        UnpaidReason::Ended => Unpaid::Ended {
            ran_out: said.ran_out,
        },
        UnpaidReason::Unconfirmed => Unpaid::Unconfirmed,
    }
}
