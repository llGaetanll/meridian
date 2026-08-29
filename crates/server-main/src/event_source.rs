//! A source answered, or failed to.

use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;

use crate::LinkPoll;

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

    /// A Link session is open, and this is the page that hosts it.
    LinkOpened {
        user_id: uuid::Uuid,
        link_token: String,
        hosted_link_url: String,
    },

    /// What every Link session polled together has come to, one
    /// [`LinkPoll`] each and in the order they were asked about.
    LinksPolled { polls: Vec<LinkPoll> },

    /// A bank is attached. Carries no access token: that stays in the runtime,
    /// beside the credentials it belongs with.
    ItemLinked { link_token: String, item_id: String },

    /// The Link flow did not produce a bank. Dismissed, failed, or timed out —
    /// the common ending, not a rare one.
    LinkAbandoned { link_token: String },
}
