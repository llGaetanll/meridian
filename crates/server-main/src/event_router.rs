//! A route was asked for something.
//!
//! One variant per route that dispatches. "Router" rather than "client": the
//! phone is a client too, but only a handler is waiting on the answer.

use crate::ConnId;
use crate::rate_limit::Route;

#[derive(Debug)]
pub enum EventRouter {
    /// Someone wants their snapshot.
    SnapshotRequested { token_sha256: Vec<u8> },

    /// Somebody wants a sign-in code for this address. Carries no code: the
    /// machine makes it, from the seeded RNG.
    CodeRequested { email: String },

    /// Somebody typed a code.
    CodeSubmitted { email: String, code: String },

    /// Somebody wants to connect a bank.
    ///
    /// The **hash** of their bearer token, never the token: events are written
    /// to the log, and a session token in one is a way into somebody's bank.
    LinkRequested { token_sha256: Vec<u8> },

    /// Somebody wants to be told how their sync is going, for as long as it
    /// goes on.
    ///
    /// A request like any other, so a caller with no session is refused before
    /// any of the response has gone out. The id is the stream its answer
    /// opens, minted at the edge beside the request's own.
    SyncWatchRequested {
        conn_id: ConnId,
        token_sha256: Vec<u8>,
    },

    /// The bank sign-in page closed, so whatever happened there has happened.
    LinkClosed { token_sha256: Vec<u8> },

    /// Plaid called us, or somebody says they are Plaid.
    ///
    /// The bytes exactly as they arrived: Plaid hashes what it put on the
    /// wire, so a body through a JSON extractor and printed again hashes to
    /// something else.
    WebhookReceived { jwt: String, body: Vec<u8> },
}

impl EventRouter {
    /// The session this request has to be resolved against before anything is
    /// decided, if it carries one.
    ///
    /// Derived from the variant rather than named by the handler, so a route
    /// added later cannot forget to be authenticated — this match will not
    /// compile until it says whether it is.
    pub fn token(&self) -> Option<&[u8]> {
        match self {
            EventRouter::LinkRequested { token_sha256 }
            | EventRouter::LinkClosed { token_sha256 }
            | EventRouter::SyncWatchRequested { token_sha256, .. }
            | EventRouter::SnapshotRequested { token_sha256 } => Some(token_sha256),

            EventRouter::CodeRequested { .. }
            | EventRouter::CodeSubmitted { .. }
            | EventRouter::WebhookReceived { .. } => None,
        }
    }

    /// Which bucket a request for this counts against.
    ///
    /// Derived from the variant rather than named by the handler, so a route
    /// added later cannot forget to be limited.
    ///
    /// `None` is exempt: a webhook comes from addresses we do not choose on a
    /// schedule we do not control, and a limit set slightly wrong there breaks
    /// bank linking silently. The signature is what protects it instead.
    pub fn route(&self) -> Option<Route> {
        match self {
            EventRouter::SnapshotRequested { .. } => Some(Route::Snapshot),
            EventRouter::CodeRequested { .. } => Some(Route::AuthRequest),
            EventRouter::CodeSubmitted { .. } => Some(Route::AuthVerify),
            EventRouter::LinkRequested { .. } => Some(Route::LinkStart),
            EventRouter::LinkClosed { .. } => Some(Route::LinkFinish),
            EventRouter::SyncWatchRequested { .. } => Some(Route::SyncStatus),
            EventRouter::WebhookReceived { .. } => None,
        }
    }

    /// Who a request for this would put mail in front of, and `None` for the
    /// routes that write to nobody.
    ///
    /// Derived from the variant for the same reason [`route`](Self::route) is:
    /// a route that learns to send mail later cannot forget to be metered.
    pub fn recipient(&self) -> Option<&str> {
        match self {
            EventRouter::CodeRequested { email } => Some(email),

            EventRouter::SnapshotRequested { .. }
            | EventRouter::CodeSubmitted { .. }
            | EventRouter::LinkRequested { .. }
            | EventRouter::LinkClosed { .. }
            | EventRouter::SyncWatchRequested { .. }
            | EventRouter::WebhookReceived { .. } => None,
        }
    }
}
