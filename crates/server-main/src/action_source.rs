//! Something for a source to do.

use crate::CallId;

#[derive(Debug)]
pub enum ActionSource {
    /// Fetch the key a webhook says it was signed with.
    ///
    /// The one call an unauthenticated request can cause.
    FetchWebhookKey { call_id: CallId, kid: String },

    /// Go and ask Plaid about one Item.
    ///
    /// Names the Item and not the access token: an action carrying one would
    /// put a bank credential in the event log. The cursor is where to resume
    /// from, and losing it means re-fetching an entire history.
    FetchPlaid {
        call_id: CallId,
        item_id: String,
        cursor: Option<String>,
    },

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
