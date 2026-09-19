//! Something for Stripe to do.

use uuid::Uuid;

#[derive(Debug)]
pub enum ActionSourceStripe {
    /// Create the customer a subscription will hang off.
    ///
    /// The address goes with it so Stripe can send a receipt, and the user id
    /// so a row in their dashboard says which of ours it is.
    CreateCustomer { user_id: Uuid, email: String },

    /// Start a subscription against the price this build was configured with.
    ///
    /// Answers with the secret the card form confirms against. Nothing is
    /// charged: the customer has given no card yet.
    CreateSubscription { user_id: Uuid, customer_id: String },

    /// Open a Checkout Session: a page of Stripe's that sells the
    /// subscription, against a customer we already hold.
    ///
    /// Answers with the page's address. Nothing is charged and nothing is
    /// created by opening it: the subscription exists once the page takes
    /// the money, and the webhook is what says so.
    OpenCheckout {
        user_id: Uuid,
        customer_id: String,

        /// Where Stripe sends the window when the money is taken — a page on
        /// the web app's origin that closes itself.
        success_url: String,

        /// The same, for backing out of the page without paying.
        cancel_url: String,
    },

    /// Read back the subscription this user already has.
    ///
    /// Asked before making a second one. A subscription somebody started and
    /// walked away from is still waiting to be paid, and its invoice still
    /// has a secret a card form can confirm against — so the answer decides
    /// whether this press picks that one up, replaces a dead one, or tells
    /// somebody they are already paying.
    LoadSubscription {
        user_id: Uuid,
        subscription_id: String,
    },

    /// Stop the subscription at the end of the period already paid for.
    ///
    /// `cancel_at_period_end`, not a delete: the terms promise the period is
    /// served out, and taking the app away the moment somebody cancels is
    /// taking something they have paid for.
    CancelSubscription {
        user_id: Uuid,
        subscription_id: String,

        /// The machine's clock, for reading the date out of what Stripe
        /// answers. Carried rather than read at the call, because a runtime
        /// that read one would be a second clock — see `docs/ARCHITECTURE.md`.
        now: api_common::Timestamp,
    },
}
