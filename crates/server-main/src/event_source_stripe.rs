//! Stripe answered, or failed to.

use uuid::Uuid;

use crate::CallId;

#[derive(Debug)]
pub enum EventSourceStripe {
    /// A customer exists, and this is what Stripe calls them.
    CustomerCreated {
        call_id: CallId,
        user_id: Uuid,
        customer_id: String,
    },

    /// A subscription exists and is waiting to be paid for.
    ///
    /// `client_secret` is what the card form needs. Absent is a subscription
    /// with nothing to pay, which on a freshly created one means Stripe
    /// answered in a shape this build did not expect.
    SubscriptionCreated {
        call_id: CallId,
        user_id: Uuid,
        subscription_id: String,
        status: String,
        client_secret: Option<String>,
    },

    /// A call to Stripe produced no answer — it refused, or it was never made.
    StripeFailed {
        call_id: CallId,
        user_id: Uuid,
        message: String,
    },
}
