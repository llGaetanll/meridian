//! The machine every test in this directory starts from, and the one fixture
//! it cannot write out on a `<` line: a webhook signature Plaid would have
//! made.
//!
//! Everything here is a helper *around* a test, which is what `docs/STYLE.md`
//! allows — the machine a test starts from, and a signature it will believe.
//! Nothing here builds an event: those are written out where they are sent.
//!
//! It is a directory module rather than a file so Cargo does not compile it as
//! a test binary of its own. Each file takes what it needs and ignores the
//! rest, which is what the allow at the top is for.

#![allow(dead_code)]

use api_common::Timestamp;
use server_main::*;

/// 2026-08-19 00:00 UTC — the clock the whole suite is set to, and the same
/// one the phone's tests use. It reaches the machine as a tick and nowhere
/// else.
pub const T0: Timestamp = Timestamp::from_secs(1_787_097_600);

/// A machine that has been told what time it is and that its process is up —
/// the first two events the runtime sends, and neither of them any test's
/// claim. What it has *not* been told is a seed: the tests that mint anything
/// send one.
///
/// It collects Link sessions by polling, which is what a laptop does and what
/// `docs/API_FLOW.md` calls C1.
pub fn state() -> StateServer {
    started(PlaidCollect::Poll)
}

/// The same machine, told that Plaid calls us instead — C2.
///
/// The channel is a fact from the edge, on the event that says the process is
/// up, so this is the whole difference between the two — and the reason
/// `/link/finish` reads rather than polls is a decision made from it.
pub fn state_collecting_by_webhook() -> StateServer {
    started(PlaidCollect::Webhook)
}

fn started(collect: PlaidCollect) -> StateServer {
    let mut state = StateServer::new();

    state.handle_events(
        &[
            Event::FromProcess {
                event: EventProcess::Tick { now: T0 },
            },
            Event::FromProcess {
                event: EventProcess::Started { collect },
            },
        ],
        &mut Vec::new(),
    );

    state
}

/// The signing key these tests pretend is Plaid's, fixed so a run is a replay.
const WEBHOOK_KEY: [u8; 32] = [
    0x4c, 0x0b, 0x30, 0x92, 0x69, 0x2c, 0x0a, 0x1e, 0x66, 0x0c, 0x4d, 0x4a, 0x37, 0x1e, 0x5b, 0x20,
    0x51, 0x2f, 0x0f, 0x36, 0x3c, 0x1e, 0x5a, 0x44, 0x2b, 0x1a, 0x3d, 0x0e, 0x77, 0x25, 0x18, 0x63,
];

pub const WEBHOOK_KID: &str = "kid-1";

/// The public half, as Plaid would answer for it.
pub fn webhook_jwk() -> src_plaid::PlaidJwk {
    use base64::Engine;

    let signing = p256::ecdsa::SigningKey::from_bytes(&WEBHOOK_KEY.into()).expect("a valid scalar");
    let point = signing.verifying_key().to_encoded_point(false);

    let b64 = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);

    src_plaid::PlaidJwk {
        alg: "ES256".to_string(),
        crv: "P-256".to_string(),
        kid: WEBHOOK_KID.to_string(),
        kty: "EC".to_string(),
        x: b64(point.x().expect("an uncompressed point")),
        y: b64(point.y().expect("an uncompressed point")),
    }
}

/// A webhook signed the way Plaid signs one: the token and the bytes it is
/// about.
///
/// What the `<` lines are about is what the machine does with a webhook it
/// believes, and a signature it will believe is the fixture that gets them
/// there — but the token has to be real, because verifying it is one of the
/// things being tested.
pub fn signed(body: &str, iat: u64) -> (String, Vec<u8>) {
    use base64::Engine;
    use p256::ecdsa::signature::Signer;
    use sha2::Digest;

    let b64 = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);

    let header = b64(format!(r#"{{"alg":"ES256","kid":"{WEBHOOK_KID}"}}"#).as_bytes());

    let digest = sha2::Sha256::digest(body.as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();

    let claims = b64(format!(r#"{{"iat":{iat},"request_body_sha256":"{hex}"}}"#).as_bytes());

    let signing = p256::ecdsa::SigningKey::from_bytes(&WEBHOOK_KEY.into()).expect("a valid scalar");
    let signature: p256::ecdsa::Signature = signing.sign(format!("{header}.{claims}").as_bytes());

    (
        format!("{header}.{claims}.{}", b64(&signature.to_bytes())),
        body.as_bytes().to_vec(),
    )
}
