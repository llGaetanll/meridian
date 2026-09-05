//! Something for a source to do.

use crate::CallId;

#[derive(Debug)]
pub enum ActionSource {
    /// Fetch the key a webhook says it was signed with.
    ///
    /// The one call an unauthenticated request can cause, which is why the
    /// machine holds what it already knows about each `kid` and asks for one
    /// at most once. The answer is
    /// [`EventSource::WebhookKeyLoaded`](crate::EventSource).
    FetchWebhookKey { call_id: CallId, kid: String },

    /// Go and ask Plaid about one Item.
    ///
    /// Names the Item rather than leaving the runtime to pick one. The access
    /// token is the runtime's — it never enters an action, because an action
    /// that carried one would put a bank credential in the event log — so this
    /// is how the runtime knows *which* token to reach for.
    ///
    /// The cursor is where to resume from, if there is one. Losing it means
    /// re-fetching an entire history.
    FetchPlaid {
        call_id: CallId,
        item_id: String,
        cursor: Option<String>,
    },

    /// Open a Link session and get the page that hosts it.
    ///
    /// Creates nothing at Plaid's end — a session nobody finishes costs
    /// nothing and expires on its own — so this is the one call in the flow
    /// that is free to repeat.
    OpenLink {
        call_id: CallId,
        user_id: uuid::Uuid,
    },

    /// Ask what became of every Link session this user has open.
    ///
    /// The completion redirect closes the sheet and carries no token, so the
    /// result is collected rather than delivered. See `docs/PLAID.md`.
    ///
    /// All of them in one action, rather than one action each, because the
    /// route asked one question — "did any of my sessions land a bank?" — and
    /// one question gets one answer. Fanning out to N calls is the runtime's
    /// business, and doing it here would be N answers to a handler that can
    /// only send one.
    PollLinks {
        call_id: CallId,
        link_tokens: Vec<String>,
    },

    /// Trade a `public_token` for the permanent one.
    ///
    /// **The call that spends a Production Item slot**, and the only action in
    /// the server whose effect cannot be undone. The access token it yields
    /// stays in the runtime: it never comes back as an event, because an event
    /// carrying one would put a bank credential in the log.
    ExchangePublicToken {
        call_id: CallId,
        link_token: String,
        public_token: String,
    },
}
