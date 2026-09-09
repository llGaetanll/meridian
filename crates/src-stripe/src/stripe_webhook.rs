//! Whether a POST to the webhook route is really Stripe's.
//!
//! The route carries no session and cannot: Stripe is not signed in as
//! anybody. The signature is the whole of what stands between a stranger and
//! a free subscription, so every check here is a refusal by default.
//!
//! Stripe signs the raw bytes of the body, so the check has to happen before
//! anything parses or re-serialises them. A body round-tripped through
//! `serde_json` is a body whose signature no longer matches.

use hmac::Hmac;
use hmac::Mac;
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// How far out of step with Stripe a signature may be and still be believed.
///
/// Bounds a replay: a captured webhook stops being useful five minutes after
/// it was signed. Stripe's own libraries use the same figure.
pub const TOLERANCE_SECS: i64 = 5 * 60;

/// Why a webhook was not believed.
///
/// Separate variants because they read differently in a log — a malformed
/// header is somebody probing, a stale timestamp is usually a clock — but they
/// all come to the same thing: it is refused.
#[derive(Debug, PartialEq, Eq)]
pub enum StripeSignatureError {
    /// No `Stripe-Signature`, or not in the shape Stripe sends.
    Malformed,

    /// Signed too long ago, or in the future.
    Stale,

    /// The header parsed and no signature in it matched.
    Mismatch,
}

impl std::fmt::Display for StripeSignatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            StripeSignatureError::Malformed => write!(f, "the signature header is not one"),
            StripeSignatureError::Stale => write!(f, "the signature is outside the window"),
            StripeSignatureError::Mismatch => write!(f, "no signature matched"),
        }
    }
}

/// One `Stripe-Signature` header, taken apart.
///
/// `v1` is a list because Stripe sends one per endpoint secret during a secret
/// roll, and the webhook is genuine if *any* of them matches.
#[derive(Debug)]
pub struct StripeSignature {
    pub timestamp: i64,
    v1: Vec<String>,
}

impl StripeSignature {
    /// Read `t=1234,v1=abc,v1=def`. Unknown schemes are skipped rather than
    /// refused, so a `v2` Stripe adds later does not break a `v1` that works.
    pub fn parse(header: &str) -> Result<Self, StripeSignatureError> {
        let mut timestamp = None;
        let mut v1 = Vec::new();

        for part in header.split(',') {
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };

            match key.trim() {
                "t" => timestamp = value.trim().parse::<i64>().ok(),
                "v1" => v1.push(value.trim().to_string()),
                _ => {}
            }
        }

        let Some(timestamp) = timestamp else {
            return Err(StripeSignatureError::Malformed);
        };

        if v1.is_empty() {
            return Err(StripeSignatureError::Malformed);
        }

        Ok(Self { timestamp, v1 })
    }
}

/// Whether `body` was signed by `secret`, and recently enough.
///
/// `now` is passed in rather than read, so this is a decision a state machine
/// can make and a test can replay. Every argument is a fact; nothing here
/// reads a clock or a socket.
pub fn verify(
    body: &[u8],
    signature: &StripeSignature,
    secret: &str,
    now: i64,
) -> Result<(), StripeSignatureError> {
    // Both directions. A timestamp far in the future is as much a forgery as
    // one far in the past, and `abs` says so without a second branch.
    if (now - signature.timestamp).abs() > TOLERANCE_SECS {
        return Err(StripeSignatureError::Stale);
    }

    // The timestamp is inside what is signed, so it cannot be edited to bring
    // a captured webhook back into the window.
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
        .map_err(|_| StripeSignatureError::Mismatch)?;

    mac.update(signature.timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);

    let expected = mac.finalize().into_bytes();

    // Constant time, and over the decoded bytes rather than the hex: a
    // comparison that stops at the first wrong character tells an attacker how
    // much of a guess was right.
    let matched = signature.v1.iter().any(|candidate| {
        let Ok(candidate) = decode_hex(candidate) else {
            return false;
        };

        candidate.ct_eq(&expected[..]).into()
    });

    if matched {
        Ok(())
    } else {
        Err(StripeSignatureError::Mismatch)
    }
}

/// Hex to bytes. `None` for anything that is not an even run of hex digits,
/// which is a signature that was never going to match.
fn decode_hex(hex: &str) -> Result<Vec<u8>, ()> {
    if !hex.len().is_multiple_of(2) {
        return Err(());
    }

    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ()))
        .collect()
}
