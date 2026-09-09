//! `POST /v1/customers` — the Stripe side of one user.

use crate::StripeClient;
use crate::StripeCustomer;
use crate::StripeError;

/// Create the customer a subscription will hang off.
///
/// `user_id` goes into metadata so that a row in Stripe's dashboard can be
/// traced back to a row in `users` without a lookup table. The address is what
/// Stripe emails a receipt to.
pub async fn create_customer(
    client: &StripeClient,
    user_id: &str,
    email: &str,
) -> Result<StripeCustomer, StripeError> {
    let form = vec![
        ("email".to_string(), email.to_string()),
        ("metadata[user_id]".to_string(), user_id.to_string()),
    ];

    client.post("customers", &form).await
}
