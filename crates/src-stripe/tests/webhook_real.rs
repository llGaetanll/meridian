//! The check, against a webhook Stripe actually signed.
//!
//! `webhook_signature.rs` builds its headers with the same HMAC the code under
//! test uses, so it proves the arithmetic is self-consistent and nothing more.
//! It would pass just as happily if the scheme were misread — the wrong bytes
//! signed, or the separator missing. This one cannot: the fixture is a real
//! delivery, captured off `stripe listen`, and the only way to accept it is to
//! have read Stripe's scheme right.
//!
//! The secret is not in the fixture. It is a live credential, and one in a
//! committed file is one in the repository forever, so it comes from the
//! environment and the test says what to do when it is absent.

use src_stripe::StripeSignature;
use src_stripe::StripeWebhookObject;
use src_stripe::verify;

const BODY: &[u8] = include_bytes!("fixtures/webhook_body.json");
const HEADER: &str = include_str!("fixtures/webhook_signature.txt");

/// The secret that signed the fixture, or a reason there is nothing to check.
///
/// A `stripe listen` prints a *new* secret each time, so a fixture and a
/// secret from different runs do not go together — which is a mismatch rather
/// than a bug in the code under test.
fn secret() -> Option<String> {
    let _ = dotenvy::dotenv();

    std::env::var("STRIPE_WEBHOOK_SECRET")
        .ok()
        .filter(|secret| !secret.is_empty())
}

#[test]
fn a_webhook_stripe_signed_is_believed() {
    let Some(secret) = secret() else {
        eprintln!("skipped: STRIPE_WEBHOOK_SECRET is not set");
        return;
    };

    let signature =
        StripeSignature::parse(HEADER.trim()).expect("stripe sent a well formed header");

    // The timestamp is the one in the fixture, so the window is not what is
    // under test here and the check does not rot the day after it was written.
    let now = signature.timestamp;

    assert_eq!(
        verify(BODY, &signature, &secret, now),
        Ok(()),
        "a real Stripe signature was refused — the scheme has been misread, \
         or the fixture and STRIPE_WEBHOOK_SECRET are from different \
         `stripe listen` runs"
    );
}

/// The same delivery, one byte different. Proves the fixture passes above
/// because the signature is right rather than because the check is lax.
#[test]
fn the_same_webhook_with_a_byte_changed_is_refused() {
    let Some(secret) = secret() else {
        eprintln!("skipped: STRIPE_WEBHOOK_SECRET is not set");
        return;
    };

    let signature =
        StripeSignature::parse(HEADER.trim()).expect("stripe sent a well formed header");

    let mut tampered = BODY.to_vec();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;

    assert!(verify(&tampered, &signature, &secret, signature.timestamp).is_err());
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

    // The fixture is a `customer.created`, which is none of the shapes this
    // app acts on — and the catch-all is exactly what must happen to it.
    assert!(matches!(event.data.object, StripeWebhookObject::Other));
}
