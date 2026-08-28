//! Something for the store to do.
//!
//! A query is IO, so it is an action out and an event back, the same as a trip
//! to Plaid. What it is *not* is a lookup on the request path: the server holds
//! its ledger in memory and answers `/snapshot` from what it holds. Postgres is
//! where that survives a restart, so the writes happen as data lands and the
//! read happens once, at boot.
//!
//! Nothing here carries an access token. The machine has never seen one — it
//! lives in the runtime, beside the credentials it belongs with — and an action
//! that carried it would put a bank credential into the event log.

use api_ledger::Account;
use api_ledger::Entry;
use api_ledger::Stream;

#[derive(Debug)]
pub enum ActionStore {
    /// Read everything back. Asked for once, when the server starts.
    LoadEverything,

    /// Write what a sync just produced.
    ///
    /// Carries the diff rather than asking the runtime to read it off state,
    /// because by the time this is performed the diff is gone — state holds the
    /// result of applying it. The clone is one per sync, next to a network
    /// call that cost far more.
    SaveFetched {
        item_id: String,

        /// What the bank is called, so a cold start has a name to draw before
        /// the first fetch comes back.
        institution_name: Option<String>,

        added: Vec<Entry>,
        modified: Vec<Entry>,
        removed: Vec<String>,

        accounts: Vec<Account>,
        streams: Vec<Stream>,

        /// Written last and separately, so a failure earlier in the batch
        /// leaves the cursor where it was and the next sync re-reads rather
        /// than skipping.
        cursor: String,
    },

    /// Remember that a connection needs a person, or that it no longer does.
    ///
    /// Its own action because it outlives the process: a server that restarts
    /// into an `ITEM_LOGIN_REQUIRED` it has forgotten will cheerfully ask Plaid
    /// again, and be told the same thing.
    SaveStatus {
        item_id: String,
        error_code: Option<String>,
    },
}
