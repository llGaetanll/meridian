//! Where a request is, between the event that asked and the answer it gets.

use uuid::Uuid;

use crate::Authenticated;
use crate::ClientIp;
use crate::Owed;

/// One route in flight, and the step it is at.
///
/// The variant is the step. A request is at exactly one of these from the
/// turn it arrives to the turn it is answered, so the table of them is the
/// list of routes in flight, and a route that is in it after its answer has
/// gone out is a leak by definition.
///
/// A step holds only what the machine cannot get back from the next event:
/// which call is out is in [`Waiting`](crate::Waiting), keyed by the call, and
/// the answer to a read arrives on the event that carries it.
#[derive(Debug)]
pub enum Pending {
    /// A sign-in, waiting on the code the store holds against its address.
    Submitted { email: String, code: String },

    /// A request waiting on the session that says who is asking. The address
    /// rides along because the rate limit needs both halves of the caller.
    Authenticating { ip: ClientIp, event: Authenticated },

    /// A webhook waiting on the key it says it was signed with. Held whole,
    /// because nothing about it can be believed until the key arrives.
    WebhookKey { jwt: String, body: Vec<u8> },

    /// A believed webhook waiting on the bank it names.
    WebhookItem { item_id: String },

    /// A believed `SESSION_FINISHED` waiting to hear whether its session
    /// repairs an Item. Held whole, because what is done with the tokens —
    /// exchanged, or deliberately not — is decided by the answer.
    WebhookLink {
        link_token: String,
        public_tokens: Vec<String>,
        exited: bool,
    },

    /// A request whose caller is known, waiting on the rows it decides from.
    Loading { user_id: Uuid, event: Authenticated },

    /// Waiting on Plaid for the page that hosts a Link session.
    Opening,

    /// Waiting on Plaid to say what the caller's open Link sessions came to.
    ///
    /// `repairs` is which of them repair an Item rather than attach one,
    /// `link_token` to `item_id`. Held here because the poll's answer does not
    /// say, and a repair's finish must not be exchanged.
    Polling {
        repairs: std::collections::BTreeMap<String, String>,
    },

    /// Waiting on Plaid to trade a `public_token` for a bank, on a route that
    /// asked for the exchange itself.
    Exchanging,

    /// Parked on a Link session Plaid has not reported on. No call is out:
    /// what it waits for arrives unprompted, or the tick lets it go.
    Watching,

    /// Waiting on the ledger it is about to be answered with.
    Reading,

    /// Waiting on Stripe to create the customer a subscription hangs off.
    Subscribing { user_id: Uuid },

    /// Waiting on Stripe to create the subscription itself.
    SubscribingTo { user_id: Uuid, customer_id: String },

    /// On the way to a Checkout page, waiting on Stripe for what comes before
    /// it: the customer the page sells against, or the read-back of a
    /// subscription already held. The addresses ride along because the page,
    /// once asked for, is told where to send its window back.
    CheckingOut {
        user_id: Uuid,
        success_url: String,
        cancel_url: String,
    },

    /// Waiting on Stripe for the Checkout page itself.
    OpeningCheckout { user_id: Uuid },

    /// A deletion, waiting on the code the store holds against its account.
    /// The same step [`Submitted`](Pending::Submitted) is for a sign-in,
    /// keyed by the user because the route already knows who is asking.
    DeleteSubmitted { user_id: Uuid, code: String },

    /// Deleting an account, waiting on Stripe to stop the subscription.
    ///
    /// First because it is the only step that can be told to somebody: a card
    /// that keeps being charged after the app is gone is the failure a person
    /// notices, where a Plaid Item they never see is one only we pay for.
    Deleting { user_id: Uuid },

    /// The subscription is stopped. Waiting on the store for the tokens the
    /// bank removals need, which have to be read before anything is deleted.
    DeletingReadingTokens { user_id: Uuid },

    /// The banks are being stopped at Plaid, `left` of them still out.
    ///
    /// Counted rather than listed, because what happens next does not depend
    /// on which bank answered: the rows go once none is outstanding. A bank
    /// that failed to be removed is logged and does not hold the deletion up
    /// — the alternative is an account nobody can delete.
    DeletingBanks { user_id: Uuid, left: u32 },

    /// A bars request waiting on the coverage that says which of its window
    /// is already held. The ask is carried whole, because the plan is decided
    /// by the answer.
    PlanningBars {
        symbol: String,
        interval: src_postgres::BarInterval,
        start: api_common::Timestamp,
        end: api_common::Timestamp,
    },

    /// A bars request parked on the fetches that fill its window, `left` of
    /// them still out. Counted rather than listed, the way
    /// [`DeletingBanks`](Pending::DeletingBanks) counts: the bars are read
    /// once none is outstanding, whichever order they land in.
    AwaitingBars {
        symbol: String,
        interval: src_postgres::BarInterval,
        start: api_common::Timestamp,
        end: api_common::Timestamp,
        left: u32,
    },

    /// A bars request waiting on the read it is about to be answered with.
    /// The interval rides along because the answer names what was served.
    ServingBars {
        symbol: String,
        interval: src_postgres::BarInterval,
    },

    /// Decided. Held until nothing the request started is still out, because
    /// a route reads the store the moment it is answered.
    Owed(Owed),
}
