//! Every call this crate makes, against Stripe's test mode, in order.
//!
//! ```sh
//! cargo run -p src-stripe --example stripe_live
//! ```
//!
//! Not a test, and deliberately not one: it needs a key, it needs the network,
//! and it leaves two objects behind. What it is for is the class of bug no unit
//! test in here can reach — a shape Stripe sends that the types do not accept.
//!
//! It has already earned that. `get_subscription` and `cancel_subscription`
//! both failed on the first run, because Stripe answers `latest_invoice` as a
//! bare id unless the call expanded it, and the type insisted on the object.
//! Every test passed, because they all used the create, which expands it.
//! `tests/expandable.rs` is that bug written down; this is what found it.
//!
//! Run it after changing a request, a response type, or `API_VERSION`.
//!
//! The objects it leaves are a customer and a subscription in test mode. They
//! cost nothing and charge nobody, and the last line says how to remove them.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    let credentials = src_stripe::StripeCredentials::from_env()?;
    let client = src_stripe::StripeClient::new(&credentials)?;

    println!("api version {}", src_stripe::API_VERSION);
    println!("price       {}", client.price_id());

    // The two calls `/billing/subscribe` makes, in the order it makes them.
    let customer =
        src_stripe::create_customer(&client, "stripe-live-example", "livecheck@example.com")
            .await?;

    println!("\ncustomer    {}", customer.id);

    let created = src_stripe::create_subscription(&client, &customer.id).await?;

    println!("subscription {}", created.id);
    println!("  status     {}", created.status);

    // The two the machine actually decides from. Both must be present on a
    // freshly created subscription, and `period_end` is not where it looks
    // like it should be — see `StripeSubscription::current_period_end`.
    println!(
        "  period end {}",
        match created.current_period_end() {
            Some(end) => end.to_string(),
            None => "MISSING — entitlement would read as expired".to_string(),
        }
    );

    println!(
        "  secret     {}",
        match created.client_secret() {
            Some(_) => "present".to_string(),
            None => "MISSING — there would be no card form".to_string(),
        }
    );

    // A plain read, which is the one that does *not* expand the invoice.
    let read = src_stripe::get_subscription(&client, &created.id).await?;

    println!("\nre-read     {} {}", read.id, read.status);
    println!(
        "  period end {}",
        match read.current_period_end() {
            Some(end) => end.to_string(),
            None => "MISSING".to_string(),
        }
    );

    let cancelled = src_stripe::cancel_subscription(&client, &created.id).await?;

    println!("\ncancelled   {}", cancelled.id);
    println!("  at period end {}", cancelled.cancel_at_period_end);
    println!(
        "  status        {}  (still active until the period ends, which is the point)",
        cancelled.status
    );

    println!("\nleft behind. To remove:");
    println!(
        "  curl -X DELETE https://api.stripe.com/v1/subscriptions/{} -u \"$STRIPE_SECRET_KEY:\"",
        created.id
    );
    println!(
        "  curl -X DELETE https://api.stripe.com/v1/customers/{} -u \"$STRIPE_SECRET_KEY:\"",
        customer.id
    );

    Ok(())
}
