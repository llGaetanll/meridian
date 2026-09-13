//! Stripe, as the server's actions reach it.
//!
//! Plain async functions over plain data, called by the runtime and by nothing
//! that decides anything. Everything here needs the Stripe secret key, which
//! can start a charge against any customer on the account and so cannot ship
//! inside an app.
//!
//! One call each, on the one [`client`] the process holds.

use src_stripe::StripeClient;
use src_stripe::StripeCredentials;
use src_stripe::StripeError;

/// The one client the process holds.
///
/// Built once at startup rather than per request, so a key that is not one is
/// a server that does not come up rather than a route that fails later.
pub fn client(credentials: StripeCredentials) -> Result<StripeClient, StripeError> {
    StripeClient::new(&credentials)
}

/// The customer a subscription will hang off.
pub async fn create_customer(
    client: &StripeClient,
    user_id: uuid::Uuid,
    email: &str,
) -> Result<String, StripeError> {
    let customer = src_stripe::create_customer(client, &user_id.to_string(), email).await?;

    Ok(customer.id)
}

/// A subscription, and what its card form confirms against.
///
/// Nothing is charged: the customer has given no card yet.
pub async fn create_subscription(
    client: &StripeClient,
    customer_id: &str,
) -> Result<Subscription, StripeError> {
    let subscription = src_stripe::create_subscription(client, customer_id).await?;

    Ok(Subscription {
        id: subscription.id.clone(),
        status: subscription.status(),
        client_secret: subscription.client_secret().map(str::to_string),
    })
}

/// Read one back, for a press of Subscribe by somebody who already started.
///
/// The same shape as creating one, because what the machine decides from is
/// the same three facts either way.
pub async fn get_subscription(
    client: &StripeClient,
    subscription_id: &str,
) -> Result<Subscription, StripeError> {
    let subscription = src_stripe::get_subscription(client, subscription_id).await?;

    Ok(Subscription {
        id: subscription.id.clone(),
        status: subscription.status(),
        client_secret: subscription.client_secret().map(str::to_string),
    })
}

/// A newly created subscription, reduced to what the machine acts on.
pub struct Subscription {
    pub id: String,
    pub status: src_stripe::StripeStatus,

    /// What the card form confirms against. `None` is a subscription with
    /// nothing to pay, which on a freshly created one is Stripe answering in a
    /// shape this build did not expect.
    pub client_secret: Option<String>,
}

/// Stop the subscription at the end of the period already paid for.
///
/// Answers when that is, which is when access actually stops.
pub async fn cancel_subscription(
    client: &StripeClient,
    subscription_id: &str,
    now: i64,
) -> Result<Option<i64>, StripeError> {
    let subscription = src_stripe::cancel_subscription(client, subscription_id).await?;

    Ok(subscription.good_until(now))
}
