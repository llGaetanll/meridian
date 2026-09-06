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

    /// Plaid answered, about one Item.
    ///
    /// The transaction lists are a **diff** — what changed since the cursor —
    /// so a second sync a minute later legitimately answers with three empty
    /// lists.
    PlaidLoaded {
        call_id: CallId,
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

    /// One stage of a fetch has landed, and this is what is known so far.
    ///
    /// A fact, not an answer: it carries no `call_id` because nothing is
    /// waiting on it. The whole fetch still answers once, with
    /// [`PlaidLoaded`](EventSource::PlaidLoaded).
    PlaidProgressed {
        item_id: String,
        stage: server_wire::SyncStage,

        /// What the bank is called, absent until the balances call has said.
        institution: Option<String>,

        accounts: u32,
        transactions: u32,
        streams: u32,
    },

    /// Plaid did not answer, or answered with an error. `code` is Plaid's own
    /// `error_code` where there was one.
    PlaidFailed {
        call_id: CallId,
        item_id: String,
        code: String,
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
