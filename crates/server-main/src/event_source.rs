//! A source answered, or failed to.

use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;

/// `PlaidLoaded` carries a whole sync and `PlaidFailed` carries a code, which
/// clippy notices. Deliberate: the diff has to reach the store somehow, and
/// boxing it would trade one allocation for a level of indirection on the path
/// that already made three network calls.
#[expect(
    clippy::large_enum_variant,
    reason = "a sync is large and has to cross; the alternative buys nothing"
)]
#[derive(Debug)]
pub enum EventSource {
    /// Plaid answered, about one Item.
    ///
    /// The transaction lists are a **diff** — what changed since the cursor —
    /// so a second sync a minute later legitimately answers with three empty
    /// lists. They go to the store, which applies them to what it holds; there
    /// is nothing in the machine to assign them over, which is what retired
    /// the trap in `docs/PLAID.md` rather than guarding against it.
    PlaidLoaded {
        item_id: String,
        added: Vec<Entry>,
        modified: Vec<Entry>,
        removed: Vec<String>,

        /// The bank and its accounts, as the balances call reported them.
        /// Optional because the balances half of a fetch can fail on its own.
        connection: Option<Connection>,

        streams: Vec<Stream>,
        cursor: String,
    },

    /// Plaid did not answer, or answered with an error. `code` is Plaid's own
    /// `error_code` where there was one.
    PlaidFailed { item_id: String, code: String },
}
