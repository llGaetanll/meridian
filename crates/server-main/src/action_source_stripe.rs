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
