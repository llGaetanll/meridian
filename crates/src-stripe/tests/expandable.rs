//! A subscription parses whether or not the call expanded its invoice.
//!
//! Stripe answers every expandable field two ways: a call that asked to expand
//! it gets the object, and one that did not gets the id as a bare string. A
//! type insisting on the object parses the create — which expands it — and
//! then fails on every plain read of the same subscription, which is a
//! `get_subscription` and a `cancel_subscription` that never work.
//!
//! Both bodies here are the shapes Stripe actually sent, trimmed to the fields
//! this crate reads.

use src_stripe::StripeSubscription;

/// What the create answers, with `expand[]=latest_invoice.confirmation_secret`.
const EXPANDED: &str = r#"{
  "id": "sub_1",
  "status": "incomplete",
  "customer": "cus_1",
  "cancel_at_period_end": false,
  "items": { "data": [ { "current_period_start": 0, "current_period_end": 1791636480 } ] },
  "latest_invoice": {
    "id": "in_1",
    "confirmation_secret": { "client_secret": "pi_1_secret" }
  }
}"#;

/// What a plain read answers: the same field, as an id.
const UNEXPANDED: &str = r#"{
  "id": "sub_1",
  "status": "incomplete",
  "customer": "cus_1",
  "cancel_at_period_end": false,
  "items": { "data": [ { "current_period_start": 0, "current_period_end": 1791636480 } ] },
  "latest_invoice": "in_1"
}"#;

#[test]
fn an_expanded_invoice_carries_the_secret() {
    let subscription: StripeSubscription =
        serde_json::from_str(EXPANDED).expect("the expanded shape parses");

    assert_eq!(subscription.client_secret(), Some("pi_1_secret"));
    assert_eq!(subscription.current_period_end(), Some(1791636480));
}

/// The one that was broken: an id where the object would be.
#[test]
fn an_invoice_that_is_only_an_id_still_parses() {
    let subscription: StripeSubscription =
        serde_json::from_str(UNEXPANDED).expect("the unexpanded shape parses");

    // No secret to give, and that is not a failure: a read of a subscription
    // is not a card form.
    assert_eq!(subscription.client_secret(), None);

    // Everything entitlement is decided from is still here.
    assert_eq!(subscription.current_period_end(), Some(1791636480));
    assert_eq!(subscription.status, "incomplete");
}

/// A subscription with no invoice at all — settled, or never billed.
#[test]
fn no_invoice_at_all_is_not_a_failure() {
    let body = r#"{
      "id": "sub_1",
      "status": "active",
      "customer": "cus_1",
      "cancel_at_period_end": false,
      "items": { "data": [ { "current_period_start": 0, "current_period_end": 1791636480 } ] }
    }"#;

    let subscription: StripeSubscription =
        serde_json::from_str(body).expect("an absent field parses");

    assert_eq!(subscription.client_secret(), None);
    assert_eq!(subscription.current_period_end(), Some(1791636480));
}
