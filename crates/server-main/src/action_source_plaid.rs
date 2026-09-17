//! Something for Plaid to do.

use crate::LinkOpener;

#[derive(Debug)]
pub enum ActionSourcePlaid {
    /// Fetch the key a webhook says it was signed with.
    ///
    /// The one call an unauthenticated request can cause.
    FetchWebhookKey { kid: String },

    /// Ask what one bank's accounts hold.
    ///
    /// The first call of a sync, and the only one that names the bank — the
    /// institution is in neither the Link session nor the exchange.
    ///
    /// Names the Item and not the access token: an action carrying one would
    /// put a bank credential in the event log. The same goes for the two
    /// below.
    FetchBalances {
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
        item_id: String,
        cursor: Option<String>,
    },

    /// Ask which charges on a bank come back.
    ///
    /// The last call of a sync, and the one it can do without: Plaid derives
    /// the streams from transactions, so a bank attached seconds ago has none.
    FetchRecurring { item_id: String },

    /// Open a Link session and get the page that hosts it.
    ///
    /// A session nobody finishes costs nothing and expires on its own, so this
    /// is the one call in the flow that is free to repeat.
    ///
    /// `opener` is who it is for, which is where Plaid sends them back and
    /// whether an OAuth bank may hand back to the app.
    OpenLink {
        user_id: uuid::Uuid,
        opener: LinkOpener,
    },

    /// Open an update-mode Link session against a bank that has stopped
    /// answering, and get the page that hosts it.
    ///
    /// Names the Item and not its access token, like every action here: the
    /// runtime holds the credential and this says which one to use. A repair
    /// leaves the token as it was and creates no Item, so this is as free to
    /// repeat as [`OpenLink`](ActionSourcePlaid::OpenLink).
    OpenRepair {
        user_id: uuid::Uuid,
        item_id: String,
        opener: LinkOpener,
    },

    /// Ask what became of every Link session this user has open.
    ///
    /// All of them in one action: fanning out to N calls is the runtime's
    /// business, and N actions would be N answers to a handler that can only
    /// send one.
    PollLinks { link_tokens: Vec<String> },

    /// Trade a `public_token` for the permanent one.
    ///
    /// **The call that spends a Production Item slot**, and the only action in
    /// the server whose effect cannot be undone.
    ExchangePublicToken {
        link_token: String,
        public_token: String,
    },

    /// Stop this bank, and stop being billed for it.
    ///
    /// The token rides on the action rather than being looked up when the call
    /// is made, because by then the row it came from is on its way out — see
    /// `src_postgres::load_access_tokens`.
    RemoveItem {
        item_id: String,
        access_token: String,
    },
}
