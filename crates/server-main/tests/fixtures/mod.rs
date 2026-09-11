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
use src_apple::AppleEnvironment;

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

/// The machine, with [`SUBSCRIBER`] paid up well past the end of the suite.
///
/// Most flows are about somebody who already has an account, and an account is
/// something you pay for — so without this every one of them is answered
/// `PaymentRequired` and nothing else happens. Entitlement arrives the way it
/// does at startup, as the store's rows, rather than by reaching into the
/// machine: a test that set a private field would be testing a state the
/// server cannot actually be in.
pub fn state_subscribed() -> StateServer {
    subscribed(state())
}

/// The same, collecting by webhook.
pub fn state_subscribed_by_webhook() -> StateServer {
    subscribed(state_collecting_by_webhook())
}

/// The user every flow test signs in as.
pub const SUBSCRIBER: uuid::Uuid = uuid::Uuid::nil();

/// Long past `T0`, so no test has to think about when the period ends. The
/// ones that *are* about that set their own.
pub const PAID_UNTIL: Timestamp = Timestamp::from_secs(T0.as_secs() + 365 * 24 * 60 * 60);

/// The machine, with several users paid up.
///
/// For a test about two accounts, where both have to be able to reach the app
/// before it can say anything about keeping them apart.
pub fn state_subscribed_many(users: &[uuid::Uuid]) -> StateServer {
    let mut state = state();

    let subscriptions: Vec<_> = users
        .iter()
        .map(|user_id| subscription(*user_id, "active", Some(PAID_UNTIL)))
        .collect();

    state.handle_events(
        &[Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded { subscriptions },
        }],
        &mut Vec::new(),
    );

    state
}

/// Hand the machine one paid-up subscription, as the startup read would.
pub fn subscribed(mut state: StateServer) -> StateServer {
    state.handle_events(
        &[Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![subscription(SUBSCRIBER, "active", Some(PAID_UNTIL))],
            },
        }],
        &mut Vec::new(),
    );

    state
}

/// What Stripe calls the subscription in these tests. Fixed rather than
/// derived from the user, so a webhook body can name it in plain sight.
pub const SUBSCRIPTION_ID: &str = "sub_1";

/// One row, as the store holds it.
pub fn subscription(
    user_id: uuid::Uuid,
    status: &str,
    current_period_end: Option<Timestamp>,
) -> src_postgres::StoredSubscription {
    src_postgres::StoredSubscription {
        user_id,
        provider: src_postgres::BillingProvider::Stripe,
        customer_id: format!("cus_{user_id}"),
        subscription_id: Some(SUBSCRIPTION_ID.to_string()),
        status: status.to_string(),
        current_period_end,
        cancel_at_period_end: false,
        last_event_id: None,
        last_event_at: None,
    }
}

fn started(collect: PlaidCollect) -> StateServer {
    let mut state = StateServer::new();

    state.handle_events(
        &[
            Event::FromProcess {
                event: EventProcess::Tick { now: T0 },
            },
            Event::FromProcess {
                event: EventProcess::Started {
                    collect,
                    stripe_webhook_secret: Some(STRIPE_WEBHOOK_SECRET.to_string()),
                    apple_environment: Some(AppleEnvironment::Sandbox),
                },
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

/// The secret these tests pretend Stripe signs with. Fixed, so a run is a
/// replay.
pub const STRIPE_WEBHOOK_SECRET: &str = "whsec_test_fixture";

/// A `Stripe-Signature` header for this body, as Stripe would write one.
///
/// The timestamp is inside what is signed, which is what stops a captured
/// delivery being edited back into its window.
pub fn stripe_signed(body: &[u8], at: Timestamp) -> String {
    use hmac::Mac;

    let mut mac =
        <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(STRIPE_WEBHOOK_SECRET.as_bytes())
            .expect("hmac takes a key of any length");

    mac.update(at.as_secs().to_string().as_bytes());
    mac.update(b".");
    mac.update(body);

    let hex: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    format!("t={},v1={hex}", at.as_secs())
}

/// A `customer.subscription.deleted` body, as Stripe sends one.
///
/// The shape that matters: `canceled`, `ended_at` set to the moment it
/// stopped, and `current_period_end` still a month out. Believing the period
/// end here serves a deleted subscription for the rest of its month.
pub fn stripe_subscription_deleted(
    event_id: &str,
    created: u64,
    subscription_id: &str,
    ended_at: u64,
    current_period_end: u64,
) -> Vec<u8> {
    format!(
        r#"{{"id":"{event_id}","type":"customer.subscription.deleted","created":{created},"data":{{"object":{{"object":"subscription","id":"{subscription_id}","status":"canceled","customer":"cus_1","cancel_at_period_end":false,"ended_at":{ended_at},"items":{{"data":[{{"current_period_start":0,"current_period_end":{current_period_end}}}]}}}}}}}}"#
    )
    .into_bytes()
}

/// A `customer.subscription.updated` body, as Stripe sends one.
///
/// `current_period_end` sits on the item rather than on the subscription,
/// which is where Stripe actually puts it — a fixture with it at the top level
/// would pass against a reader that looks in the wrong place.
pub fn stripe_subscription_event(
    event_id: &str,
    created: u64,
    subscription_id: &str,
    status: &str,
    current_period_end: u64,
) -> Vec<u8> {
    format!(
        r#"{{"id":"{event_id}","type":"customer.subscription.updated","created":{created},"data":{{"object":{{"object":"subscription","id":"{subscription_id}","status":"{status}","customer":"cus_1","cancel_at_period_end":false,"items":{{"data":[{{"current_period_start":0,"current_period_end":{current_period_end}}}]}}}}}}}}"#
    )
    .into_bytes()
}
