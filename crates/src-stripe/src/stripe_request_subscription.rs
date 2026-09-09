//! `POST /v1/subscriptions` — start one, and get back what the card form needs.

use crate::StripeClient;
use crate::StripeError;
use crate::StripeSubscription;

/// Start a subscription that is not paid for yet.
///
/// Three parameters do the work, and each is load-bearing:
///
/// - `payment_behavior=default_incomplete` leaves it `incomplete` rather than
///   trying to charge a card the customer has not given yet. Without it Stripe
///   fails the call for want of a payment method.
/// - `expand[]=latest_invoice.confirmation_secret` brings back the secret the
///   card form confirms against, which is otherwise a second round trip.
/// - `save_default_payment_method=on_subscription` makes the card that pays
///   the first invoice the one that pays the renewals. Without it every month
///   after the first fails.
pub async fn create_subscription(
    client: &StripeClient,
    customer_id: &str,
) -> Result<StripeSubscription, StripeError> {
    let form = vec![
        ("customer".to_string(), customer_id.to_string()),
        ("items[0][price]".to_string(), client.price_id().to_string()),
        (
            "payment_behavior".to_string(),
            "default_incomplete".to_string(),
        ),
        (
            "payment_settings[save_default_payment_method]".to_string(),
            "on_subscription".to_string(),
        ),
        (
            "expand[]".to_string(),
            "latest_invoice.confirmation_secret".to_string(),
        ),
    ];

    client.post("subscriptions", &form).await
}

/// Read one back. What the webhook says is the truth; this is for a route that
/// needs the current answer without waiting for one.
pub async fn get_subscription(
    client: &StripeClient,
    subscription_id: &str,
) -> Result<StripeSubscription, StripeError> {
    client
        .get(&format!("subscriptions/{subscription_id}"))
        .await
}

/// Stop one at the end of the period already paid for.
///
/// `cancel_at_period_end` rather than a delete: the terms say a cancelled
/// subscription runs to the end of the period, and deleting it would take the
/// app away from somebody who has paid for the rest of the month.
pub async fn cancel_subscription(
    client: &StripeClient,
    subscription_id: &str,
) -> Result<StripeSubscription, StripeError> {
    let form = vec![("cancel_at_period_end".to_string(), "true".to_string())];

    client
        .post(&format!("subscriptions/{subscription_id}"), &form)
        .await
}
