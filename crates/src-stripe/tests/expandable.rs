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

/// The two ways a subscription stops, told apart.
///
/// `current_period_end` is not the answer on its own: a subscription that has
/// ended still reports the period it had reached, so a deleted one looks paid
/// up for the rest of the month. `ended_at` is what says otherwise, and
/// `good_until` is the one a caller should ask.
mod ending {
    use src_stripe::StripeSubscription;

    /// The clock a caller would pass in. Well after the periods below, so a
    /// case that answers `now` is unmistakable.
    const NOW: i64 = 5_000;

    /// Cancelled at period end. Still running, still paid for.
    const CANCEL_AT_PERIOD_END: &str = r#"{
      "id": "sub_1",
      "status": "active",
      "customer": "cus_1",
      "cancel_at_period_end": true,
      "ended_at": null,
      "items": { "data": [ { "current_period_start": 0, "current_period_end": 2000 } ] }
    }"#;

    /// Deleted. The shape Stripe actually sends on
    /// `customer.subscription.deleted`: `canceled`, `ended_at` now, and a
    /// period end a month out.
    const DELETED: &str = r#"{
      "id": "sub_1",
      "status": "canceled",
      "customer": "cus_1",
      "cancel_at_period_end": false,
      "ended_at": 1000,
      "items": { "data": [ { "current_period_start": 0, "current_period_end": 2000 } ] }
    }"#;

    #[test]
    fn cancelling_at_period_end_keeps_the_period() {
        let subscription: StripeSubscription =
            serde_json::from_str(CANCEL_AT_PERIOD_END).expect("it parses");

        assert_eq!(subscription.good_until(NOW), Some(2000));
        assert!(subscription.cancel_at_period_end);
    }

    /// The bug this was written for: believing `current_period_end` here
    /// serves a deleted subscription for the rest of its month.
    #[test]
    fn a_subscription_that_ended_is_good_until_it_ended() {
        let subscription: StripeSubscription = serde_json::from_str(DELETED).expect("it parses");

        assert_eq!(
            subscription.good_until(NOW),
            Some(1000),
            "a subscription that ended is not good until the period it never finished"
        );

        // The period end is still there, and is still the wrong thing to ask.
        assert_eq!(subscription.current_period_end(), Some(2000));
    }

    /// An older delivery, or a version without the field, is not a failure.
    #[test]
    fn an_absent_ended_at_falls_back_to_the_period() {
        let body = r#"{
          "id": "sub_1",
          "status": "active",
          "customer": "cus_1",
          "cancel_at_period_end": false,
          "items": { "data": [ { "current_period_start": 0, "current_period_end": 2000 } ] }
        }"#;

        let subscription: StripeSubscription = serde_json::from_str(body).expect("it parses");

        assert_eq!(subscription.good_until(NOW), Some(2000));
    }

    /// Stripe says outright to revoke access on `unpaid`: the retries are
    /// already exhausted. Nothing ended, so `ended_at` is null, and the
    /// period can even have rolled forward — invoices keep being generated
    /// while nobody pays them.
    #[test]
    fn an_unpaid_subscription_is_worth_nothing_from_now() {
        let body = r#"{
          "id": "sub_1",
          "status": "unpaid",
          "customer": "cus_1",
          "cancel_at_period_end": false,
          "ended_at": null,
          "items": { "data": [ { "current_period_start": 0, "current_period_end": 9000 } ] }
        }"#;

        let subscription: StripeSubscription = serde_json::from_str(body).expect("it parses");

        assert_eq!(
            subscription.good_until(NOW),
            Some(NOW),
            "an unpaid subscription is not good until a period nobody paid for"
        );
    }

    /// A first payment that has not landed. Stripe already reports the period
    /// a month out on the `created` event, and believing that opens the app
    /// for somebody who has paid nothing.
    #[test]
    fn an_incomplete_subscription_has_no_date_at_all() {
        let body = r#"{
          "id": "sub_1",
          "status": "incomplete",
          "customer": "cus_1",
          "cancel_at_period_end": false,
          "items": { "data": [ { "current_period_start": 0, "current_period_end": 9000 } ] }
        }"#;

        let subscription: StripeSubscription = serde_json::from_str(body).expect("it parses");

        assert_eq!(subscription.good_until(NOW), None);
    }

    /// A first payment that never landed. Terminal, and it never bought
    /// anything.
    #[test]
    fn an_expired_first_payment_is_worth_nothing() {
        let body = r#"{
          "id": "sub_1",
          "status": "incomplete_expired",
          "customer": "cus_1",
          "cancel_at_period_end": false,
          "items": { "data": [ { "current_period_start": 0, "current_period_end": 9000 } ] }
        }"#;

        let subscription: StripeSubscription = serde_json::from_str(body).expect("it parses");

        assert_eq!(subscription.good_until(NOW), Some(NOW));
    }

    /// `past_due` is **not** revoked, and that is deliberate. Stripe is still
    /// retrying, the period was paid for, and the ordinary cause is an expired
    /// card on somebody who will replace it. Revoking here takes the app from
    /// a paying customer over something a smart retry fixes within the hour.
    #[test]
    fn a_past_due_subscription_keeps_the_period_it_paid_for() {
        let body = r#"{
          "id": "sub_1",
          "status": "past_due",
          "customer": "cus_1",
          "cancel_at_period_end": false,
          "ended_at": null,
          "items": { "data": [ { "current_period_start": 0, "current_period_end": 9000 } ] }
        }"#;

        let subscription: StripeSubscription = serde_json::from_str(body).expect("it parses");

        assert_eq!(subscription.good_until(NOW), Some(9000));
    }

    /// A status nobody has seen before is not a reason to lock somebody out.
    /// Stripe adds statuses; the date it reports is still the date.
    #[test]
    fn a_status_this_build_does_not_know_falls_back_to_the_period() {
        let body = r#"{
          "id": "sub_1",
          "status": "something_stripe_added_later",
          "customer": "cus_1",
          "cancel_at_period_end": false,
          "items": { "data": [ { "current_period_start": 0, "current_period_end": 9000 } ] }
        }"#;

        let subscription: StripeSubscription = serde_json::from_str(body).expect("it parses");

        assert_eq!(subscription.good_until(NOW), Some(9000));
    }
}
