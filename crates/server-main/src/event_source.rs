//! A source answered, or failed to.

use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;

use crate::CallId;
use crate::LinkPoll;

#[derive(Debug)]
pub enum EventSource {
    /// What Plaid says about one signing key. `None` is an id it would not
    /// answer for — invented, or a real one we failed to fetch.
    WebhookKeyLoaded {
        call_id: CallId,
        kid: String,
        key: Option<src_plaid::PlaidJwk>,
    },

    /// Plaid said what one bank's accounts hold.
    ///
    /// The bank's name arrives on this and nowhere else, which is why it is
    /// the call a sync starts with.
    BalancesLoaded {
        call_id: CallId,
        item_id: String,
        connection: Connection,
    },

    /// One page of what changed on a bank since the cursor it was asked with.
    ///
    /// The three lists are a **diff** — what changed — so a sync a minute
    /// after the last one legitimately answers with all three empty.
    TransactionsLoaded {
        call_id: CallId,
        item_id: String,
        added: Vec<Entry>,
        modified: Vec<Entry>,
        removed: Vec<String>,

        /// Where to resume: the next page's cursor, or the next sync's.
        cursor: String,

        /// Whether Plaid has another page behind this one.
        has_more: bool,
    },

    /// The charges Plaid says come back on this bank. Empty is ordinary on a
    /// bank whose history has only just landed.
    RecurringLoaded {
        call_id: CallId,
        item_id: String,
        streams: Vec<Stream>,
    },

    /// Plaid did not answer one call, or answered it with an error.
    ///
    /// `stage` is which of a sync's calls this was, because what a failure
    /// costs depends on that: the recurring one is one screen and the other
    /// two are the sync.
    PlaidFailed {
        call_id: CallId,
        item_id: String,
        stage: server_wire::SyncStage,
        fault: src_plaid::PlaidFault,
    },

    /// A Link session is open, and this is the page that hosts it.
    LinkOpened {
        call_id: CallId,
        user_id: uuid::Uuid,
        link_token: String,
        hosted_link_url: String,
    },

    /// What every Link session polled together has come to, one
    /// [`LinkPoll`] each and in the order they were asked about.
    LinksPolled {
        call_id: CallId,
        polls: Vec<LinkPoll>,
    },

    /// A bank is attached. Carries no access token: that stays in the runtime,
    /// beside the credentials it belongs with.
    ItemLinked {
        call_id: CallId,
        link_token: String,
        item_id: String,
    },

    /// The Link flow did not produce a bank. Dismissed, failed, or timed out —
    /// the common ending, not a rare one.
    LinkAbandoned { call_id: CallId, link_token: String },
}
