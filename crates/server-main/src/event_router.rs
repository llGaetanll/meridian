//! A route was asked for something.
//!
//! One variant per route that dispatches. "Router" rather than "client": the
//! phone is a client too, but only a handler is waiting on the answer.

use crate::ConnId;
use crate::rate_limit::Route;

#[derive(Debug)]
pub enum EventRouter {
    /// Someone wants their snapshot.
    SnapshotRequested { token_sha256: Vec<u8> },

    /// Somebody wants a sign-in code for this address. Carries no code: the
    /// machine makes it, from the seeded RNG.
    CodeRequested { email: String },

    /// Somebody typed a code.
    CodeSubmitted { email: String, code: String },

    /// Somebody wants to connect a bank.
    ///
    /// The **hash** of their bearer token, never the token: events are written
    /// to the log, and a session token in one is a way into somebody's bank.
    ///
    /// `origin` is the request's `Origin` header, which a browser sends and
    /// the phone does not. Present but unreadable arrives as whatever it
    /// decodes to, never as absent: absent is the phone.
    LinkRequested {
        token_sha256: Vec<u8>,
        origin: Option<String>,
    },

    /// Somebody wants a bank they already connected repaired — Plaid's update
    /// mode, for an Item whose login has expired. Names the Item, which is the
    /// one thing [`LinkRequested`](EventRouter::LinkRequested) does not carry.
    RepairRequested {
        token_sha256: Vec<u8>,
        origin: Option<String>,
        item_id: String,
    },

    /// Somebody wants to be told how their sync is going, for as long as it
    /// goes on.
    ///
    /// A request like any other, so a caller with no session is refused before
    /// any of the response has gone out. The id is the stream its answer
    /// opens, minted at the edge beside the request's own.
    SyncWatchRequested {
        conn_id: ConnId,
        token_sha256: Vec<u8>,
    },

    /// The bank sign-in page closed, so whatever happened there has happened.
    LinkClosed { token_sha256: Vec<u8> },

    /// Plaid called us, or somebody says they are Plaid.
    ///
    /// The bytes exactly as they arrived: Plaid hashes what it put on the
    /// wire, so a body through a JSON extractor and printed again hashes to
    /// something else.
    WebhookReceived { jwt: String, body: Vec<u8> },

    /// `POST /billing/subscribe` — start a subscription for whoever this token
    /// signs in.
    SubscribeRequested { token_sha256: Vec<u8> },

    /// `POST /billing/checkout` — open a page of Stripe's that sells the
    /// subscription, for whoever this token signs in.
    ///
    /// `origin` plays the part it does on
    /// [`LinkRequested`](EventRouter::LinkRequested): it is where the page's
    /// window is sent back, and only an origin the web app is served from is
    /// one.
    CheckoutRequested {
        token_sha256: Vec<u8>,
        origin: Option<String>,
    },

    /// `GET /billing/status` — what this user's subscription is.
    BillingStatusRequested { token_sha256: Vec<u8> },

    /// `POST /account/name` — what to call whoever this token signs in.
    ///
    /// First run asks once and this is that answer; nothing else writes it
    /// yet, and a Google sign-in will hand one over without asking.
    NameSubmitted { token_sha256: Vec<u8>, name: String },

    /// `POST /stripe/webhook`.
    ///
    /// The body is raw bytes because that is what the signature is over: a
    /// body round-tripped through a parser is a body whose signature no
    /// longer matches.
    StripeWebhookReceived { signature: String, body: Vec<u8> },

    /// `POST /billing/apple` — the App Store took a payment, and this is
    /// Apple's word for it.
    ///
    /// Carries a token because the transaction says which Apple ID paid and
    /// not which of our accounts did; the session is the only thing that ties
    /// the two together.
    ///
    /// A string rather than raw bytes, unlike the two webhooks above. Apple
    /// signs the JWS itself and not the body that carried it, so the envelope
    /// can be parsed at the edge and only what was signed comes through.
    AppleTransactionReceived {
        token_sha256: Vec<u8>,
        signed: String,
    },

    /// `POST /apple/webhook` — what Apple says happened to a subscription
    /// while nobody was looking.
    ///
    /// No session: Apple is not signed in as anybody.
    AppleNotificationReceived { signed: String },

    /// `POST /account/delete/request` — email whoever this token signs in a
    /// code that confirms the deletion. Carries no code: the machine makes
    /// it, from the seeded RNG.
    DeleteCodeRequested { token_sha256: Vec<u8> },

    /// `POST /account/delete` — cancel the subscription, stop the banks, and
    /// delete everything, if the code is the one that was emailed.
    DeleteRequested { token_sha256: Vec<u8>, code: String },

    /// `GET /bars` — one symbol's bars over one window, at one interval.
    ///
    /// The window is half-open, `start..end`. Answered from the store where
    /// it is covered, and held until the fetches that fill it land where it
    /// is not.
    BarsRequested {
        token_sha256: Vec<u8>,
        symbol: String,
        interval: src_postgres::BarInterval,
        start: api_common::Timestamp,
        end: api_common::Timestamp,
    },
}

impl EventRouter {
    /// The session this request has to be resolved against before anything is
    /// decided, if it carries one.
    ///
    /// Derived from the variant rather than named by the handler, so a route
    /// added later cannot forget to be authenticated — this match will not
    /// compile until it says whether it is.
    pub fn token(&self) -> Option<&[u8]> {
        match self {
            EventRouter::LinkRequested { token_sha256, .. }
            | EventRouter::RepairRequested { token_sha256, .. }
            | EventRouter::LinkClosed { token_sha256 }
            | EventRouter::SyncWatchRequested { token_sha256, .. }
            | EventRouter::SnapshotRequested { token_sha256 }
            | EventRouter::SubscribeRequested { token_sha256 }
            | EventRouter::CheckoutRequested { token_sha256, .. }
            | EventRouter::BillingStatusRequested { token_sha256 }
            | EventRouter::NameSubmitted { token_sha256, .. }
            | EventRouter::AppleTransactionReceived { token_sha256, .. }
            | EventRouter::DeleteCodeRequested { token_sha256 }
            | EventRouter::DeleteRequested { token_sha256, .. }
            | EventRouter::BarsRequested { token_sha256, .. } => Some(token_sha256),

            EventRouter::CodeRequested { .. }
            | EventRouter::CodeSubmitted { .. }
            | EventRouter::WebhookReceived { .. }
            | EventRouter::AppleNotificationReceived { .. }
            | EventRouter::StripeWebhookReceived { .. } => None,
        }
    }

    /// Which bucket a request for this counts against.
    ///
    /// Derived from the variant rather than named by the handler, so a route
    /// added later cannot forget to be limited.
    ///
    /// `None` is exempt: a webhook comes from addresses we do not choose on a
    /// schedule we do not control, and a limit set slightly wrong there breaks
    /// bank linking silently. The signature is what protects it instead.
    pub fn route(&self) -> Option<Route> {
        match self {
            EventRouter::SnapshotRequested { .. } => Some(Route::Snapshot),
            EventRouter::CodeRequested { .. } => Some(Route::AuthRequest),
            EventRouter::CodeSubmitted { .. } => Some(Route::AuthVerify),
            EventRouter::LinkRequested { .. } => Some(Route::LinkStart),
            EventRouter::RepairRequested { .. } => Some(Route::LinkRepair),
            EventRouter::LinkClosed { .. } => Some(Route::LinkFinish),
            EventRouter::SyncWatchRequested { .. } => Some(Route::SyncStatus),
            EventRouter::SubscribeRequested { .. } => Some(Route::BillingSubscribe),
            EventRouter::CheckoutRequested { .. } => Some(Route::BillingCheckout),
            EventRouter::BillingStatusRequested { .. } => Some(Route::BillingStatus),
            EventRouter::NameSubmitted { .. } => Some(Route::AccountName),
            EventRouter::AppleTransactionReceived { .. } => Some(Route::BillingApple),
            EventRouter::DeleteCodeRequested { .. } => Some(Route::AccountDeleteRequest),
            EventRouter::DeleteRequested { .. } => Some(Route::AccountDelete),
            EventRouter::BarsRequested { .. } => Some(Route::Bars),

            EventRouter::WebhookReceived { .. }
            | EventRouter::AppleNotificationReceived { .. }
            | EventRouter::StripeWebhookReceived { .. } => None,
        }
    }

    /// Who a request for this would put mail in front of, and `None` for the
    /// routes that write to nobody.
    ///
    /// Derived from the variant for the same reason [`route`](Self::route) is:
    /// a route that learns to send mail later cannot forget to be metered.
    pub fn recipient(&self) -> Option<&str> {
        match self {
            EventRouter::CodeRequested { email } => Some(email),

            // Mails too, but its own account only: the address is not known
            // at the edge, and the per-user window is the same inbox.
            EventRouter::DeleteCodeRequested { .. } => None,

            EventRouter::SnapshotRequested { .. }
            | EventRouter::CodeSubmitted { .. }
            | EventRouter::LinkRequested { .. }
            | EventRouter::RepairRequested { .. }
            | EventRouter::LinkClosed { .. }
            | EventRouter::SyncWatchRequested { .. }
            | EventRouter::WebhookReceived { .. }
            | EventRouter::SubscribeRequested { .. }
            | EventRouter::CheckoutRequested { .. }
            | EventRouter::BillingStatusRequested { .. }
            | EventRouter::NameSubmitted { .. }
            | EventRouter::DeleteRequested { .. }
            | EventRouter::AppleTransactionReceived { .. }
            | EventRouter::AppleNotificationReceived { .. }
            | EventRouter::StripeWebhookReceived { .. }
            | EventRouter::BarsRequested { .. } => None,
        }
    }
}
