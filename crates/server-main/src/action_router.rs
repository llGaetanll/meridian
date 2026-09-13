//! The answer a route is waiting on.
//!
//! One per [`EventRouter`](crate::EventRouter): the handler parks until its
//! own comes back, so an event that produced none never answers.

use server_wire::WireUnpaid;

use crate::ConnId;

#[derive(Debug)]
pub enum ActionRouter {
    /// Answer with this user's ledger, carried whole because nothing outside
    /// the machine reads the store.
    Serve { snapshot: server_wire::WireSnapshot },

    /// The request was taken, and nothing more is owed about it. Strictly that
    /// much: not "an email was sent", and not "the webhook was acted on".
    Accepted,

    /// Nobody is signed in by the token this request carried. Which of wrong,
    /// expired or signed out is deliberately not told apart.
    Unauthorized,

    /// The machine cannot answer this yet — being asked for a credential
    /// before any entropy has arrived, which has no safe fallback.
    Unavailable,

    /// They are signed in, and this is the token that says so.
    ///
    /// The token is on the action because the machine generated it, from the
    /// seeded RNG, and this is the only place it crosses.
    SignedIn { token: String },

    /// Send them to Plaid's page to sign into their bank.
    LinkOpened { url: String },

    /// How many banks the Link flow attached, and never zero.
    ///
    /// A count and not a yes: one `/link/finish` collects *every* session the
    /// caller has open, so it can attach two. None attached is
    /// [`LinkAbandoned`](ActionRouter::LinkAbandoned) rather than a zero here.
    Linked { count: u32 },

    /// The Link session came to no bank — the ordinary ending, somebody closed
    /// the sheet, and not a failure.
    LinkAbandoned,

    /// Plaid has not said what the Link session came to. Neither that a bank
    /// was attached nor that none was, which beats guessing between the two.
    LinkPending,

    /// The stream is theirs, and this is where things stand on it.
    ///
    /// The status rides on the acceptance rather than following it, so a
    /// connection cannot be opened without being told where the sync has got
    /// to. A client that connects late, or reconnects, is then in the same
    /// position as one that was there from the start.
    Watching {
        conn_id: ConnId,
        status: server_wire::WireSyncStatus,
    },

    /// Too many requests, and this one is not being served.
    ///
    /// Carries nothing on purpose: told when the next call would be allowed, a
    /// caller behaving badly sits just inside the limit.
    TooMany,

    /// The code was not good. Deliberately carries no reason: wrong, expired
    /// and exhausted are logged and are not told apart to the caller.
    Rejected,

    /// Signed in, and has not paid. **402.**
    ///
    /// Told apart from [`Unauthorized`](ActionRouter::Unauthorized) on
    /// purpose: the session is good and the answer is a paywall, not a sign-in
    /// screen. A client that conflated them would sign somebody out for not
    /// having paid.
    ///
    /// It carries why, because one code is several screens and only the server
    /// can tell them apart — and because the provider's own vocabulary stops
    /// here (`entitlement::unpaid`) rather than reaching a client.
    PaymentRequired { unpaid: WireUnpaid },

    /// The App Store account that paid already pays for somebody else's.
    ///
    /// One payer, one account — the rule the `subscriptions` unique index
    /// keeps. Told apart from every other refusal because it is the one the
    /// person can do something about: it is the wrong account signed in, not a
    /// purchase that went wrong.
    Claimed,

    /// This person already has a subscription that is worth something.
    ///
    /// Its own refusal for the same reason [`Claimed`](Self::Claimed) is: it
    /// is not a failure, and the caller can act on it. There is nothing to
    /// sell somebody who is already paying, and selling them a second
    /// subscription would charge their card twice for one app.
    Subscribed,

    /// A subscription has been started, and this is what the card form
    /// confirms against, and the key it opens Stripe.js with.
    ///
    /// Nothing has been charged yet. Entitlement arrives by webhook, or never.
    Subscribing {
        client_secret: String,
        publishable_key: String,
    },

    /// What this user's subscription is, for the screen that draws it.
    BillingStatus {
        /// Whether the app may be used right now.
        entitled: bool,

        /// Why not, present exactly when [`entitled`](Self::BillingStatus) is
        /// false. Our word and never the provider's, the same one the 402
        /// carries — a client draws one paywall, so it reads one vocabulary.
        unpaid: Option<WireUnpaid>,

        /// The end of the period paid for, in seconds.
        good_until: Option<u64>,

        cancel_at_period_end: bool,
    },

    /// The account is gone. Answered once the rows are actually deleted, so a
    /// client that hears this can forget its session without wondering.
    Deleted,
}
