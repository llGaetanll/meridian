//! A source answered, or failed to.

use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;

#[derive(Debug)]
pub enum EventSource {
    /// Plaid answered.
    ///
    /// The transaction lists are a **diff** — what changed since the cursor —
    /// so they are applied to the held ledger rather than replacing it. A
    /// second sync a minute later legitimately returns three empty lists, and
    /// assigning that over the ledger is how the app went blank on refresh.
    /// See `docs/PLAID.md`.
    PlaidLoaded {
        added: Vec<Entry>,
        modified: Vec<Entry>,
        removed: Vec<String>,
        connections: Vec<Connection>,
        streams: Vec<Stream>,
        cursor: String,
    },

    /// Plaid did not answer, or answered with an error. `code` is Plaid's own
    /// `error_code` where there was one.
    PlaidFailed { code: String },
}
