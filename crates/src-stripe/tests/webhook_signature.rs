//! What the signature check believes, and what it refuses.
//!
//! Every case is built from a secret and a body rather than from a captured
//! header, so the arithmetic is the thing under test.

use hmac::Hmac;
use hmac::Mac;
use sha2::Sha256;

use src_stripe::StripeSignature;
use src_stripe::StripeSignatureError;
use src_stripe::TOLERANCE_SECS;
use src_stripe::verify;

const SECRET: &str = "whsec_test_secret";
const BODY: &[u8] = br#"{"id":"evt_1","type":"customer.subscription.updated"}"#;
const NOW: i64 = 1_788_988_000;

/// A header the way Stripe writes one.
fn signed(body: &[u8], secret: &str, timestamp: i64) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
        .expect("hmac takes a key of any length");

    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);

    let hex: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    format!("t={timestamp},v1={hex}")
}

/// The hex of a header's first `v1`, for a case that builds its own header.
fn v1_of(header: &str) -> String {
    header
        .split("v1=")
        .nth(1)
        .expect("the header carries a v1")
        .to_string()
}

#[test]
fn a_signature_stripe_made_is_believed() {
    let header = signed(BODY, SECRET, NOW);
    let signature = StripeSignature::parse(&header).expect("the header is well formed");

    assert_eq!(verify(BODY, &signature, SECRET, NOW), Ok(()));
}

/// The body is what is signed, so a byte of it changing is a forgery.
#[test]
fn a_body_edited_after_signing_is_refused() {
    let header = signed(BODY, SECRET, NOW);
    let signature = StripeSignature::parse(&header).expect("the header is well formed");

    let tampered = br#"{"id":"evt_1","type":"customer.subscription.deleted"}"#;

    assert_eq!(
        verify(tampered, &signature, SECRET, NOW),
        Err(StripeSignatureError::Mismatch)
    );
}

#[test]
fn a_signature_by_another_secret_is_refused() {
    let header = signed(BODY, "whsec_someone_elses", NOW);
    let signature = StripeSignature::parse(&header).expect("the header is well formed");

    assert_eq!(
        verify(BODY, &signature, SECRET, NOW),
        Err(StripeSignatureError::Mismatch)
    );
}

/// The window is what bounds a replay.
#[test]
fn a_signature_older_than_the_window_is_refused() {
    let header = signed(BODY, SECRET, NOW);
    let signature = StripeSignature::parse(&header).expect("the header is well formed");

    assert_eq!(
        verify(BODY, &signature, SECRET, NOW + TOLERANCE_SECS + 1),
        Err(StripeSignatureError::Stale)
    );
}

/// Held right up to the edge, so the boundary is a decision rather than an
/// accident of which comparison was written.
#[test]
fn a_signature_at_the_edge_of_the_window_is_believed() {
    let header = signed(BODY, SECRET, NOW);
    let signature = StripeSignature::parse(&header).expect("the header is well formed");

    assert_eq!(
        verify(BODY, &signature, SECRET, NOW + TOLERANCE_SECS),
        Ok(())
    );
}

/// A timestamp ahead of us is as much a forgery as one behind.
#[test]
fn a_signature_from_the_future_is_refused() {
    let header = signed(BODY, SECRET, NOW);
    let signature = StripeSignature::parse(&header).expect("the header is well formed");

    assert_eq!(
        verify(BODY, &signature, SECRET, NOW - TOLERANCE_SECS - 1),
        Err(StripeSignatureError::Stale)
    );
}

/// During a secret roll Stripe signs with both, and either may be ours.
#[test]
fn one_matching_signature_among_several_is_enough() {
    let hex = v1_of(&signed(BODY, SECRET, NOW));

    let header = format!(
        "t={NOW},v1=0000000000000000000000000000000000000000000000000000000000000000,v1={hex}"
    );
    let signature = StripeSignature::parse(&header).expect("the header is well formed");

    assert_eq!(verify(BODY, &signature, SECRET, NOW), Ok(()));
}

#[test]
fn a_header_without_a_timestamp_is_not_one() {
    assert_eq!(
        StripeSignature::parse("v1=abcd").unwrap_err(),
        StripeSignatureError::Malformed
    );
}

#[test]
fn a_header_without_a_signature_is_not_one() {
    assert_eq!(
        StripeSignature::parse(&format!("t={NOW}")).unwrap_err(),
        StripeSignatureError::Malformed
    );
}

/// A scheme we do not know is skipped rather than fatal, so a `v2` Stripe adds
/// later does not stop a `v1` that works.
#[test]
fn an_unknown_scheme_alongside_a_good_v1_is_ignored() {
    let hex = v1_of(&signed(BODY, SECRET, NOW));

    let header = format!("t={NOW},v0=whatever,v1={hex}");
    let signature = StripeSignature::parse(&header).expect("the header is well formed");

    assert_eq!(verify(BODY, &signature, SECRET, NOW), Ok(()));
}

/// Not hex, or an odd number of digits: refused rather than panicking on the
/// slice.
#[test]
fn a_signature_that_is_not_hex_is_refused() {
    let header = format!("t={NOW},v1=zzz");
    let signature = StripeSignature::parse(&header).expect("the header is well formed");

    assert_eq!(
        verify(BODY, &signature, SECRET, NOW),
        Err(StripeSignatureError::Mismatch)
    );
}
