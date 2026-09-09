//! Something for Stripe to do.

use crate::CallId;

use uuid::Uuid;

#[derive(Debug)]
pub enum ActionSourceStripe {
    /// Create the customer a subscription will hang off.
    ///
    /// The address goes with it so Stripe can send a receipt, and the user id
    /// so a row in their dashboard says which of ours it is.
    CreateCustomer {
        call_id: CallId,
        user_id: Uuid,
        email: String,
    },

    /// Start a subscription against the price this build was configured with.
    ///
    /// Answers with the secret the card form confirms against. Nothing is
    /// charged: the customer has given no card yet.
    CreateSubscription {
        call_id: CallId,
        user_id: Uuid,
        customer_id: String,
    },
}
