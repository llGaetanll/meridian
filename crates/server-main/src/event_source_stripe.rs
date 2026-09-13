//! Stripe answered, or failed to.

use src_stripe::StripeStatus;
use uuid::Uuid;

#[derive(Debug)]
pub enum EventSourceStripe {
    /// A customer exists, and this is what Stripe calls them.
    CustomerCreated { user_id: Uuid, customer_id: String },

    /// A subscription exists and is waiting to be paid for.
    ///
    /// `client_secret` is what the card form needs. Absent is a subscription
    /// with nothing to pay, which on a freshly created one means Stripe
    /// answered in a shape this build did not expect.
    SubscriptionCreated {
        user_id: Uuid,
        subscription_id: String,
        status: StripeStatus,
        client_secret: Option<String>,
    },

    /// The subscription this user already had, as Stripe has it now.
    ///
    /// The status is the whole point: `incomplete` is an invoice still waiting
    /// for a card, and `client_secret` is what confirms it. Anything terminal
    /// is a subscription to replace, and anything worth something means this
    /// person is already paying.
    SubscriptionLoaded {
        user_id: Uuid,
        subscription_id: String,
        status: StripeStatus,
        client_secret: Option<String>,
    },

    /// A call to Stripe produced no answer — it refused, or it was never made.
    StripeFailed { user_id: Uuid, message: String },

    /// The subscription will not renew.
    SubscriptionCancelled {
        user_id: Uuid,

        /// When the period already paid for runs out, which is when access
        /// actually stops.
        good_until: Option<u64>,
    },
}
