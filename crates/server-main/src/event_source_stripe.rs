//! Stripe answered, or failed to.

use src_stripe::StripeStatus;
use src_stripe::StripeSubscription;
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

    /// A Checkout page exists, and this is where it is.
    ///
    /// Nothing exists at Stripe beyond the page itself: the subscription it
    /// sells is created when the money is taken, and arrives by webhook.
    CheckoutOpened { user_id: Uuid, url: String },

    /// The subscription this user already had, as Stripe has it now.
    ///
    /// The whole object, because it is believed the way a webhook is: what
    /// Stripe says now is the truth about entitlement, and a webhook that
    /// never arrived is repaired by reading. Then its status is one decision:
    /// `incomplete` is an invoice still waiting for a card, anything terminal
    /// is a subscription to replace, and anything worth something means this
    /// person is already paying.
    SubscriptionLoaded {
        user_id: Uuid,
        subscription: StripeSubscription,
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
