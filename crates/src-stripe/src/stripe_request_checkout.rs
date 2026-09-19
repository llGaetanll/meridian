//! `POST /v1/checkout/sessions` — a page of Stripe's that sells the
//! subscription, so no card detail ever touches ours.

use crate::StripeCheckoutSession;
use crate::StripeClient;
use crate::StripeError;

/// Open a Checkout Session that starts a subscription when it is paid.
///
/// `customer` is ours, made first, so the webhooks the payment causes name a
/// customer this server knows. The subscription itself is Stripe's to create,
/// on the page: nothing exists until the money is taken, so a page somebody
/// closes leaves nothing behind. The card that pays becomes the
/// subscription's default on its own — Checkout does in one page what
/// `create_subscription` needs `save_default_payment_method` for.
pub async fn create_checkout_session(
    client: &StripeClient,
    customer_id: &str,
    success_url: &str,
    cancel_url: &str,
) -> Result<StripeCheckoutSession, StripeError> {
    let form = vec![
        ("mode".to_string(), "subscription".to_string()),
        ("customer".to_string(), customer_id.to_string()),
        (
            "line_items[0][price]".to_string(),
            client.price_id().to_string(),
        ),
        ("line_items[0][quantity]".to_string(), "1".to_string()),
        ("success_url".to_string(), success_url.to_string()),
        ("cancel_url".to_string(), cancel_url.to_string()),
    ];

    client.post("checkout/sessions", &form).await
}
