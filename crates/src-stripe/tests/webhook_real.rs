//! The check, against a webhook Stripe actually signed.
//!
//! `webhook_signature.rs` builds its headers with the same HMAC the code under
//! test uses, so it proves the arithmetic is self-consistent and nothing more.
//! It would pass just as happily if the scheme were misread — the wrong bytes
//! signed, or the separator missing. This one cannot: the fixture is a real
//! delivery, captured off `stripe listen`, and the only way to accept it is to
//! have read Stripe's scheme right.
//!
//! # The secret is here, on purpose
//!
//! It is the one `stripe listen` printed for the run that produced this body,
//! and it stopped meaning anything the moment that process exited: a CLI
//! listener's secret is per-run, is not the account's, and is not what a
//! registered endpoint signs with. So it is inert, and keeping it beside the
//! fixture is what makes this test self-contained.
//!
//! The first version of this read `STRIPE_WEBHOOK_SECRET` from the
//! environment, which was wrong in a way worth writing down: the fixture and
//! the environment are only in step until somebody rolls the real secret — and
//! rolling it is a thing we *want* to be easy. A test that fails when a
//! credential is rotated correctly is a test that teaches people to ignore it.

use src_stripe::StripeSignature;
use src_stripe::StripeWebhookObject;
use src_stripe::verify;

const BODY: &[u8] = include_bytes!("fixtures/webhook_body.json");
const HEADER: &str = include_str!("fixtures/webhook_signature.txt");
const SECRET: &str = include_str!("fixtures/webhook_secret.txt");

/// The header, and the moment Stripe signed it.
///
/// The timestamp comes off the fixture rather than the clock, so the window is
/// not what is under test here and this does not rot the day after it is
/// written.
fn signature() -> StripeSignature {
    StripeSignature::parse(HEADER.trim()).expect("stripe sent a well formed header")
}

#[test]
fn a_webhook_stripe_signed_is_believed() {
    let signature = signature();

    assert_eq!(
        verify(BODY, &signature, SECRET.trim(), signature.timestamp),
        Ok(()),
        "a real Stripe signature was refused, which means the scheme has been misread"
    );
}

/// The same delivery, one byte different. Proves the fixture passes above
/// because the signature is right rather than because the check is lax.
#[test]
fn the_same_webhook_with_a_byte_changed_is_refused() {
    let signature = signature();

    let mut tampered = BODY.to_vec();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;

    assert!(verify(&tampered, &signature, SECRET.trim(), signature.timestamp).is_err());
}

/// And it is this secret that makes it verify, not any secret.
#[test]
fn another_secret_does_not_verify_it() {
    let signature = signature();

    assert!(
        verify(
            BODY,
            &signature,
            "whsec_something_else",
            signature.timestamp
        )
        .is_err()
    );
}

/// What Stripe sends parses into what this crate declares. Independent of the
/// signature: a body that verifies and then fails to parse is still a webhook
/// nobody can act on.
#[test]
fn a_real_delivery_parses() {
    let event: src_stripe::StripeWebhookEvent =
        serde_json::from_slice(BODY).expect("the fixture is an event");

    assert!(event.id.starts_with("evt_"));
    assert!(!event.r#type.is_empty());
    assert!(event.created > 0);

    // A `customer.subscription.updated`, which is one of the five this app
    // subscribes to — so it must reach the arm that decides entitlement rather
    // than the catch-all.
    let StripeWebhookObject::Subscription(subscription) = &event.data.object else {
        panic!(
            "a subscription event did not parse as one: {:?}",
            event.data.object
        );
    };

    // The two fields entitlement is actually decided from, on a real body.
    assert!(!subscription.status.is_empty());
    assert!(
        subscription.current_period_end().is_some(),
        "the period sits on the item, and a real delivery has it there"
    );
}
