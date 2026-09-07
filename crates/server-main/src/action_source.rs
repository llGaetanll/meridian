//! Something for a source to do.

use crate::CallId;

#[derive(Debug)]
pub enum ActionSource {
    /// Fetch the key a webhook says it was signed with.
    ///
    /// The one call an unauthenticated request can cause.
    FetchWebhookKey { call_id: CallId, kid: String },

    /// Ask what one bank's accounts hold.
    ///
    /// The first call of a sync, and the only one that names the bank — the
    /// institution is in neither the Link session nor the exchange.
    ///
    /// Names the Item and not the access token: an action carrying one would
    /// put a bank credential in the event log. The same goes for the two
    /// below.
    FetchBalances {
        call_id: CallId,
        item_id: String,

        /// Carries the clock, so the `synced_at` written against this bank is
        /// the moment the machine reckoned rather than one the runtime read
        /// behind its back — which no replay of the events could reproduce.
        now: api_common::Timestamp,
    },

    /// Ask for one page of what has changed on a bank since `cursor`.
    ///
    /// One page. Whether there is another is on the answer, and asking for it
    /// is another of these — so a history that takes four calls is four
    /// decisions rather than a loop nothing can see into.
    ///
    /// `cursor` is where to resume from, and losing it means re-fetching an
    /// entire history.
    SyncTransactions {
        call_id: CallId,
        item_id: String,
        cursor: Option<String>,
    },

    /// Ask which charges on a bank come back.
    ///
    /// The last call of a sync, and the one it can do without: Plaid derives
    /// the streams from transactions, so a bank attached seconds ago has none.
    FetchRecurring { call_id: CallId, item_id: String },

    /// Open a Link session and get the page that hosts it.
    ///
    /// A session nobody finishes costs nothing and expires on its own, so this
    /// is the one call in the flow that is free to repeat.
    OpenLink {
        call_id: CallId,
        user_id: uuid::Uuid,
    },

    /// Ask what became of every Link session this user has open.
    ///
    /// All of them in one action: fanning out to N calls is the runtime's
    /// business, and N actions would be N answers to a handler that can only
    /// send one.
    PollLinks {
        call_id: CallId,
        link_tokens: Vec<String>,
    },

    /// Trade a `public_token` for the permanent one.
    ///
    /// **The call that spends a Production Item slot**, and the only action in
    /// the server whose effect cannot be undone.
    ExchangePublicToken {
        call_id: CallId,
        link_token: String,
        public_token: String,
    },
}
