//! Becoming a subscriber, and what a webhook does about it — tested the way
//! `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! Two conversations, and they are deliberately not one. `POST
//! /billing/subscribe` is a request somebody waits on: an address read, a
//! customer, a subscription, and the secret a card form confirms against. What
//! it does *not* do is make anybody entitled — nothing has been charged when
//! the route answers, and a card that fails after the form closes is the
//! ordinary case rather than the strange one.
//!
//! Entitlement arrives on the other conversation, which nobody asked for: a
//! webhook, believed or not on its signature alone, naming a subscription
//! rather than a person. The tests that matter most here are the ones about a
//! delivery that is a repeat, a forgery, or late.
//!
//! The webhook signature is HMAC over the timestamp and the raw body, which is
//! why `fixtures::stripe_signed` exists and why a body is never rebuilt
//! between signing and sending: a body round-tripped through a parser is a
//! body whose signature no longer matches.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use fixtures::PAID_UNTIL;
use fixtures::SUBSCRIBER;
use fixtures::T0;
use fixtures::WEB_ORIGIN;
use fixtures::state;
use fixtures::state_subscribed;
use fixtures::stripe_signed;
use fixtures::stripe_subscription_event;

use server_main::*;

/// The token a session is looked up by. The value does not matter: the store
/// answers whatever the test says it does.
fn hash_token(token: &str) -> Vec<u8> {
    use sha2::Digest;

    sha2::Sha256::digest(token.as_bytes()).to_vec()
}

/// G. The whole of it: a session with no subscription, and the two calls to
/// Stripe that give it something to pay with.
///
/// The address is read first because the machine does not hold one — nothing
/// it keeps says who a user is besides their id, and Stripe emails a receipt.
#[test]
fn subscribing_reads_an_address_then_asks_stripe_twice() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadUser { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::UserLoaded {
                email: Some("al@example.com".to_string()),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CreateCustomer {
                user_id: SUBSCRIBER,
                ..
            },
        },
        < Event::FromSourceStripe {
            call_id: 3,
            event: EventSourceStripe::CustomerCreated {
                user_id: SUBSCRIBER,
                customer_id: "cus_1".to_string(),
            },
        },

        > Action::ToSourceStripe {
            call_id: 4,
            action: ActionSourceStripe::CreateSubscription {
                user_id: SUBSCRIBER,
                ..
            },
        },
        < Event::FromSourceStripe {
            call_id: 4,
            event: EventSourceStripe::SubscriptionCreated {
                user_id: SUBSCRIBER,
                subscription_id: "sub_1".to_string(),
                status: src_stripe::StripeStatus::Incomplete,
                client_secret: Some("pi_1_secret".to_string()),
            },
        },

        // The row is written whatever happens next: the webhook that says this
        // was paid for names only the subscription, so without it there is
        // nothing to look the user up by.
        > Action::ToStore {
            call_id: 5,
            action: ActionStore::SaveCustomer { user_id: SUBSCRIBER, .. },
        },
        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },

        // Answered only once that write has landed. A route is released when
        // nothing it started is still out, so the row exists before the card
        // form does.
        > @r#"ToRouter { request_id: 1, action: Subscribing { client_secret: "pi_1_secret", publishable_key: "pk_test_1" } }"#,
    );
}

/// A second press while the first is still being answered asks Stripe for
/// nothing. Both requests would otherwise see a user Stripe has never heard
/// of and make a customer and a subscription each, and the machine would
/// remember only the later one — so a card paid in the first tab would be a
/// webhook naming a subscription nobody knows.
#[test]
fn a_second_press_while_one_is_in_flight_is_refused() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadUser { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::UserLoaded {
                email: Some("al@example.com".to_string()),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CreateCustomer {
                user_id: SUBSCRIBER,
                ..
            },
        },

        // The same person, from another tab, before Stripe has answered.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 4,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToRouter { request_id: 2, action: ActionRouter::Subscribed },
    );
}

/// A process with no publishable key asks Stripe for nothing. A secret with no
/// key to open a card form with is a subscription nobody can pay, left behind
/// in Stripe.
#[test]
fn a_machine_with_no_publishable_key_starts_no_subscription() {
    let mut state = StateServer::new();

    state.handle_events(
        &[
            Event::FromProcess {
                event: EventProcess::Tick { now: T0 },
            },
            Event::FromProcess {
                event: EventProcess::Started {
                    collect: PlaidCollect::Poll,
                    web_origins: WebOrigins::parse("http://127.0.0.1:8080")
                        .expect("a literal origin"),
                    stripe_webhook_secret: None,
                    stripe_publishable_key: None,
                    apple_environment: None,
                },
            },
            Event::FromStore {
                call_id: STARTUP_READ,
                event: EventStore::SubscriptionsLoaded {
                    subscriptions: Vec::new(),
                },
            },
        ],
        &mut Vec::new(),
    );

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,
    );
}

/// The route answering is not a payment. Nothing has been charged when the
/// card form opens, so the session is still refused everything.
#[test]
fn a_started_subscription_entitles_nobody() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 2, action: ActionStore::LoadUser { .. } },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::UserLoaded {
                email: Some("al@example.com".to_string()),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CreateCustomer { .. },
        },
        < Event::FromSourceStripe {
            call_id: 3,
            event: EventSourceStripe::CustomerCreated {
                user_id: SUBSCRIBER,
                customer_id: "cus_1".to_string(),
            },
        },

        > Action::ToSourceStripe {
            call_id: 4,
            action: ActionSourceStripe::CreateSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 4,
            event: EventSourceStripe::SubscriptionCreated {
                user_id: SUBSCRIBER,
                subscription_id: "sub_1".to_string(),
                status: src_stripe::StripeStatus::Incomplete,
                client_secret: Some("pi_1_secret".to_string()),
            },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::SaveCustomer { .. },
        },
        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Subscribing { client_secret: "pi_1_secret", publishable_key: "pk_test_1" } }"#,

        // The same session, asking for the thing it pays for.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 6,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 6,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 2, action: PaymentRequired { unpaid: WireUnpaid { reason: "never", ran_out: None } } }"#,
    );
}

/// A webhook is what makes somebody entitled, and the same session then works.
#[test]
fn a_believed_webhook_is_what_opens_the_app() {
    let mut state = state();

    let body = stripe_subscription_event(
        "evt_1",
        T0.as_secs(),
        "sub_1",
        "active",
        PAID_UNTIL.as_secs(),
    );
    let signature = stripe_signed(&body, T0);

    eval!(&mut state,
        // The subscription has to be known before a webhook naming it can be
        // matched to a user, which is what the first conversation is for.
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "incomplete",
                    None,
                )],
            },
        },

        // Answered in the same turn the signature checks out, before the write
        // lands: Stripe retries anything it does not hear about.
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived {
                signature: signature.clone(),
                body: body.clone(),
            },
        },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },

        // And now the app opens.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 3, action: ActionStore::LoadItems { .. } },
    );
}

/// The `created` webhook carries `incomplete` with a period already a month
/// out, and nothing has been paid. The date alone would open the app, so an
/// `incomplete` subscription has to be worth nothing whatever period it
/// reports.
#[test]
fn an_incomplete_subscription_opens_nothing_whatever_its_period_says() {
    let mut state = state();

    let body = stripe_subscription_event(
        "evt_1",
        T0.as_secs(),
        "sub_1",
        "incomplete",
        PAID_UNTIL.as_secs(),
    );
    let signature = stripe_signed(&body, T0);

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "incomplete",
                    None,
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived {
                signature: signature.clone(),
                body: body.clone(),
            },
        },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },

        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 2, action: PaymentRequired { unpaid: WireUnpaid { reason: "unfinished", ran_out: None } } }"#,
    );
}

/// Stripe says outright that events are not delivered in order. A delivery
/// older than the one already believed says nothing new, and the machine's
/// copy holds the same rule the store's query does — otherwise the two
/// disagree until the next restart, and the machine's is the one every
/// snapshot reads.
#[test]
fn a_stale_delivery_after_a_newer_one_moves_nothing() {
    let mut state = state();

    let paid = stripe_subscription_event(
        "evt_2",
        T0.as_secs() + 60,
        "sub_1",
        "active",
        PAID_UNTIL.as_secs(),
    );
    let paid_signature = stripe_signed(&paid, T0);

    // The `created` event, a minute older, delivered late.
    let created = stripe_subscription_event(
        "evt_1",
        T0.as_secs(),
        "sub_1",
        "incomplete",
        PAID_UNTIL.as_secs(),
    );
    let created_signature = stripe_signed(&created, T0);

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "incomplete",
                    None,
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived {
                signature: paid_signature,
                body: paid,
            },
        },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },

        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived {
                signature: created_signature,
                body: created,
            },
        },

        > @r#"ToRouter { request_id: 2, action: Accepted }"#,

        // Sent anyway: the query refuses it for itself.
        > Action::ToStore {
            call_id: 2,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },

        < Event::FromRouter {
            request_id: 3,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BillingStatusRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 3, action: BillingStatus { entitled: true, unpaid: None, good_until: Some(1818633600), cancel_at_period_end: false } }"#,

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },
    );
}

/// A signature that does not check out is refused, and **nothing is written**.
///
/// What the claim is about is the action that does not come out: a forged
/// webhook must not reach the store, because the store is what entitlement is
/// read back from at the next startup.
#[test]
fn a_webhook_signed_by_somebody_else_writes_nothing() {
    let mut state = state();

    let body = stripe_subscription_event(
        "evt_1",
        T0.as_secs(),
        "sub_1",
        "active",
        PAID_UNTIL.as_secs(),
    );

    // Well formed, correctly timed, and signed with the wrong key.
    let signature = format!(
        "t={},v1=0000000000000000000000000000000000000000000000000000000000000000",
        T0.as_secs()
    );

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "incomplete",
                    None,
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived { signature, body },
        },

        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// The body is what is signed, so a byte of it changing is a forgery — even
/// where the signature was ours a moment ago.
#[test]
fn a_webhook_edited_after_signing_is_refused() {
    let mut state = state();

    let body = stripe_subscription_event(
        "evt_1",
        T0.as_secs(),
        "sub_1",
        "active",
        PAID_UNTIL.as_secs(),
    );
    let signature = stripe_signed(&body, T0);

    // The same delivery, made to say the period runs a year longer.
    let tampered = stripe_subscription_event(
        "evt_1",
        T0.as_secs(),
        "sub_1",
        "active",
        PAID_UNTIL.as_secs() + 365 * 24 * 60 * 60,
    );

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "incomplete",
                    None,
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived {
                signature,
                body: tampered,
            },
        },

        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// A signature older than the window is refused however good it is. What this
/// bounds is a replay: a captured delivery stops being useful.
#[test]
fn a_webhook_outside_its_window_is_refused() {
    let mut state = state();

    let signed_at = Timestamp::from_secs(T0.as_secs() - 6 * 60);

    let body = stripe_subscription_event(
        "evt_1",
        signed_at.as_secs(),
        "sub_1",
        "active",
        PAID_UNTIL.as_secs(),
    );
    let signature = stripe_signed(&body, signed_at);

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "incomplete",
                    None,
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived { signature, body },
        },

        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// A machine that was never told a secret refuses every webhook, which is the
/// safe direction: an unverified one is somebody claiming a payment.
#[test]
fn a_machine_with_no_secret_believes_nothing() {
    let mut state = StateServer::new();

    state.handle_events(
        &[
            Event::FromProcess {
                event: EventProcess::Tick { now: T0 },
            },
            Event::FromProcess {
                event: EventProcess::Started {
                    collect: PlaidCollect::Poll,
                    web_origins: WebOrigins::parse("http://127.0.0.1:8080")
                        .expect("a literal origin"),
                    stripe_webhook_secret: None,
                    stripe_publishable_key: None,
                    apple_environment: None,
                },
            },
            Event::FromStore {
                call_id: STARTUP_READ,
                event: EventStore::SubscriptionsLoaded {
                    subscriptions: Vec::new(),
                },
            },
        ],
        &mut Vec::new(),
    );

    let body = stripe_subscription_event(
        "evt_1",
        T0.as_secs(),
        "sub_1",
        "active",
        PAID_UNTIL.as_secs(),
    );
    let signature = stripe_signed(&body, T0);

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived { signature, body },
        },

        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// A believed webhook about a subscription this process has never heard of is
/// still taken and still written.
///
/// The row can exist in the store where this machine has not got it in hand —
/// a customer created by another process, or one from before a restart — and
/// the write is keyed by the subscription rather than by the user.
#[test]
fn a_webhook_for_an_unknown_subscription_is_still_written() {
    let mut state = state();

    let body = stripe_subscription_event(
        "evt_1",
        T0.as_secs(),
        "sub_nobody",
        "active",
        PAID_UNTIL.as_secs(),
    );
    let signature = stripe_signed(&body, T0);

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived { signature, body },
        },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },
    );
}

/// An event about something that is not a subscription is taken and acted on
/// no further. Refusing it only asks Stripe to send it again.
///
/// An invoice, because it is the one that looks as if it should count. It
/// does not: the endpoint is not subscribed to invoice events, and a renewal
/// arrives as a subscription event carrying the new period whether the
/// invoice was paid or not — so one that turns up anyway is the same shrug
/// as any other object.
#[test]
fn an_invoice_says_nothing_about_entitlement() {
    let mut state = state();

    let body = br#"{"id":"evt_1","type":"invoice.paid","created":1787097600,"data":{"object":{"object":"invoice","id":"in_1","customer":"cus_1"}}}"#.to_vec();
    let signature = stripe_signed(&body, T0);

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived { signature, body },
        },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,
    );
}

/// Cancelling does not close the app. The period paid for is what decides, and
/// `site/app/terms` promises exactly this.
#[test]
fn a_cancelled_subscription_still_opens_the_app_until_its_period_ends() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "canceled",
                    Some(PAID_UNTIL),
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },
    );
}

/// The other half of it, and the one that costs money if it is wrong: a
/// subscription Stripe still calls `active` whose period has run out does not
/// open the app.
///
/// A renewal that has not been paid leaves the status alone for a while, so a
/// check that read the status would serve somebody for free.
#[test]
fn an_active_subscription_whose_period_ran_out_does_not() {
    let mut state = state();

    let ran_out = Timestamp::from_secs(T0.as_secs() - 1);

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "active",
                    Some(ran_out),
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 1, action: PaymentRequired { unpaid: WireUnpaid { reason: "unconfirmed", ran_out: Some(1787097599) } } }"#,
    );
}

/// A subscription that was started and never paid for is refused as exactly
/// that. It is not a payment nobody has confirmed: nothing was charged, and
/// what opens the app is finishing the card form.
#[test]
fn an_unfinished_subscription_is_refused_as_unfinished() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "incomplete",
                    None,
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 1, action: PaymentRequired { unpaid: WireUnpaid { reason: "unfinished", ran_out: None } } }"#,
    );
}

/// Entitlement lapses because the clock moved, and nothing sweeps it. The same
/// user, the same row, one tick either side of the period ending.
#[test]
fn a_period_ends_because_a_tick_says_so() {
    let mut state = state();

    let ends = Timestamp::from_secs(T0.as_secs() + 60);

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "active",
                    Some(ends),
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        // Finished, so the tick below has nothing of its own to give up on.
        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::ItemsLoaded { items: vec![] },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::LoadSnapshot { .. },
        },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::SnapshotLoaded {
                snapshot: server_wire::WireSnapshot::new(&[], &[], &[]),
            },
        },

        > Action::ToRouter {
            request_id: 1,
            action: ActionRouter::Serve { .. },
        },

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },

        // A minute later, and nothing else has happened.
        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(ends.as_secs() + 1),
            },
        },

        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 5,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 2, action: PaymentRequired { unpaid: WireUnpaid { reason: "unconfirmed", ran_out: Some(1787097660) } } }"#,
    );
}

/// The two routes that lead to paying are the two an unpaid session may use.
/// Refusing them would leave somebody signed in with no way to become a
/// customer, which is the one thing the gate must not do.
#[test]
fn an_unpaid_session_can_still_ask_where_it_stands() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BillingStatusRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 1, action: BillingStatus { entitled: false, unpaid: Some(WireUnpaid { reason: "never", ran_out: None }), good_until: None, cancel_at_period_end: false } }"#,

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },
    );
}

/// And once they have paid, it says so.
#[test]
fn a_paid_session_is_told_what_it_has() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BillingStatusRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 1, action: BillingStatus { entitled: true, unpaid: None, good_until: Some(1818633600), cancel_at_period_end: false } }"#,

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },
    );
}

/// A token that names nobody is not a paywall. Told apart on purpose: one is a
/// sign-in screen and the other is a card form, and a client that conflated
/// them would sign somebody out for not having paid.
#[test]
fn a_token_that_names_nobody_is_unauthorized_rather_than_unpaid() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("stale"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: None },
        },

        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// Stripe refusing is not a subscription, and the caller is told so rather
/// than handed a secret that is not one.
#[test]
fn a_stripe_that_will_not_answer_is_not_a_subscription() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 2, action: ActionStore::LoadUser { .. } },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::UserLoaded {
                email: Some("al@example.com".to_string()),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CreateCustomer { .. },
        },
        < Event::FromSourceStripe {
            call_id: 3,
            event: EventSourceStripe::StripeFailed {
                user_id: SUBSCRIBER,
                message: "stripe said no".to_string(),
            },
        },

        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,
    );
}

/// A deleted subscription closes the app **now**, not at the end of the month
/// it had already reached.
///
/// Stripe sends `customer.subscription.deleted` with `status: canceled`,
/// `ended_at` set to the moment it stopped, and `current_period_end` still a
/// month in the future — the period it never finished. Deciding from the
/// period end serves somebody whose payment was reversed, or whose
/// subscription was deleted for fraud, for up to another month.
#[test]
fn a_deleted_subscription_closes_the_app_at_once() {
    let mut state = state();

    let ended_at = Timestamp::from_secs(T0.as_secs() + 60);

    let body = fixtures::stripe_subscription_deleted(
        "evt_1",
        ended_at.as_secs(),
        "sub_1",
        ended_at.as_secs(),
        // A month past the end, which is what Stripe still reports.
        ended_at.as_secs() + 30 * 24 * 60 * 60,
    );

    let signature = stripe_signed(&body, ended_at);

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "active",
                    Some(PAID_UNTIL),
                )],
            },
        },

        // Paid up, and the app opens.
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::ItemsLoaded { items: vec![] },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::LoadSnapshot { .. },
        },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::SnapshotLoaded {
                snapshot: server_wire::WireSnapshot::new(&[], &[], &[]),
            },
        },

        > Action::ToRouter {
            request_id: 1,
            action: ActionRouter::Serve { .. },
        },

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },

        // The subscription is deleted.
        < Event::FromProcess { event: EventProcess::Tick { now: ended_at } },

        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived { signature, body },
        },

        > @r#"ToRouter { request_id: 2, action: Accepted }"#,

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },

        // And the app is shut, in the same second — not in a month.
        < Event::FromRouter {
            request_id: 3,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 6,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 6,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 3, action: PaymentRequired { unpaid: WireUnpaid { reason: "ended", ran_out: Some(1787097660) } } }"#,
    );
}

/// A session naming a user the store does not have.
///
/// It should not happen — `sessions` is `ON DELETE CASCADE` from `users`, so a
/// live session cannot outlive its user. What matters is that the impossible
/// case answers rather than hanging: the route is let go of, and no customer
/// is created at Stripe for somebody who is not there.
#[test]
fn a_session_whose_user_is_gone_starts_nothing_at_stripe() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        // No such user. The claim is the `ToSourceStripe` that does not come.
        > Action::ToStore { call_id: 2, action: ActionStore::LoadUser { .. } },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::UserLoaded { email: None },
        },

        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// Stripe answering with a subscription that has nothing to confirm against.
///
/// It should not happen either: a freshly created subscription with
/// `payment_behavior=default_incomplete` always has an invoice waiting. If it
/// ever does, the caller is told the machine cannot answer rather than handed
/// a secret that is not one — but the row is still written, because the
/// subscription exists at Stripe whatever we were told about it.
#[test]
fn a_subscription_with_no_secret_is_still_written_down() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 2, action: ActionStore::LoadUser { .. } },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::UserLoaded {
                email: Some("al@example.com".to_string()),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CreateCustomer { .. },
        },
        < Event::FromSourceStripe {
            call_id: 3,
            event: EventSourceStripe::CustomerCreated {
                user_id: SUBSCRIBER,
                customer_id: "cus_1".to_string(),
            },
        },

        > Action::ToSourceStripe {
            call_id: 4,
            action: ActionSourceStripe::CreateSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 4,
            event: EventSourceStripe::SubscriptionCreated {
                user_id: SUBSCRIBER,
                subscription_id: "sub_1".to_string(),
                status: src_stripe::StripeStatus::Incomplete,
                client_secret: None,
            },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::SaveCustomer { .. },
        },
        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,
    );
}

/// Stripe refusing the *second* call, after the customer exists.
///
/// The first call cost something that the failure does not undo: there is a
/// customer at Stripe with no subscription against it. The route is answered
/// and dropped, and asking again is what the caller does — which is why
/// `save_customer` upserts on the user rather than inserting.
#[test]
fn a_stripe_that_refuses_the_subscription_still_answers() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 2, action: ActionStore::LoadUser { .. } },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::UserLoaded {
                email: Some("al@example.com".to_string()),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CreateCustomer { .. },
        },
        < Event::FromSourceStripe {
            call_id: 3,
            event: EventSourceStripe::CustomerCreated {
                user_id: SUBSCRIBER,
                customer_id: "cus_1".to_string(),
            },
        },

        > Action::ToSourceStripe {
            call_id: 4,
            action: ActionSourceStripe::CreateSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 4,
            event: EventSourceStripe::StripeFailed {
                user_id: SUBSCRIBER,
                message: "stripe said no".to_string(),
            },
        },

        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,
    );
}

/// A believed webhook whose body is not an event this build reads.
///
/// **Accepted, not refused.** The signature was ours, so this is an event
/// nobody asked for or a shape we do not parse, and a refusal only asks Stripe
/// to send it again — for three days, with backoff. The claim is the store
/// write that does not happen.
#[test]
fn a_believed_body_that_is_not_an_event_is_taken_and_dropped() {
    let mut state = state();

    let body = br#"{"this":"is signed, and is not an event"}"#.to_vec();
    let signature = stripe_signed(&body, T0);

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived { signature, body },
        },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,
    );
}

/// `past_due` keeps the app open, and that is a decision rather than an
/// oversight.
///
/// Stripe is still retrying, the period was paid for, and the ordinary cause
/// is an expired card on somebody who will replace it. Revoking here takes the
/// app from a paying customer over something a smart retry fixes within the
/// hour. `unpaid` is where access stops — see the next test.
#[test]
fn a_past_due_subscription_still_opens_the_app() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "past_due",
                    Some(PAID_UNTIL),
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },
    );
}

/// The one Stripe is explicit about: revoke on `unpaid`, because the retries
/// are already exhausted.
///
/// It arrives as an ordinary `customer.subscription.updated` with no
/// `ended_at` — nothing ended — and a period that may even have rolled
/// forward, because invoices keep being generated while nobody pays them.
#[test]
fn an_unpaid_subscription_closes_the_app() {
    let mut state = state();

    let body = stripe_subscription_event(
        "evt_1",
        T0.as_secs(),
        "sub_1",
        "unpaid",
        // A month out, and worth nothing.
        T0.as_secs() + 30 * 24 * 60 * 60,
    );

    let signature = stripe_signed(&body, T0);

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "active",
                    Some(PAID_UNTIL),
                )],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived { signature, body },
        },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },

        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 2, action: PaymentRequired { unpaid: WireUnpaid { reason: "declined", ran_out: Some(1787097600) } } }"#,
    );
}

/// A renewal moves the date the app is open until.
///
/// The month rolls over, Stripe sends the new period, and the same session
/// keeps working — which is the whole ordinary life of a subscription and was
/// otherwise only tested going wrong.
#[test]
fn a_renewal_moves_the_period_forward() {
    let mut state = state();

    let first_end = Timestamp::from_secs(T0.as_secs() + 60);
    let renewed_end = Timestamp::from_secs(first_end.as_secs() + 30 * 24 * 60 * 60);

    let body = stripe_subscription_event(
        "evt_renewal",
        first_end.as_secs(),
        "sub_1",
        "active",
        renewed_end.as_secs(),
    );

    let signature = stripe_signed(&body, first_end);

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "active",
                    Some(first_end),
                )],
            },
        },

        // A minute on, and the period has run out.
        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(first_end.as_secs() + 1),
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 1, action: PaymentRequired { unpaid: WireUnpaid { reason: "unconfirmed", ran_out: Some(1787097660) } } }"#,

        // The renewal lands.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived { signature, body },
        },

        > @r#"ToRouter { request_id: 2, action: Accepted }"#,

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },

        // And the app is open again, on the same session.
        < Event::FromRouter {
            request_id: 3,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 4, action: ActionStore::LoadItems { .. } },
    );
}

/// Somebody who opened the card form, walked away, and came back gets the
/// same subscription rather than a second one.
///
/// This is what #107 was: every press started a brand new subscription, and a
/// person ended up with several, all filed under them. The one they abandoned
/// expired a day later, Stripe said so, and the server read that as the truth
/// about whether they had paid.
///
/// The claim is the two actions that do not come out — no customer and no
/// subscription created — and the secret that does: the same one the first
/// press handed over, because it is the same invoice waiting to be paid.
#[test]
fn coming_back_to_an_abandoned_card_form_picks_up_the_same_subscription() {
    let mut state = state();

    eval!(&mut state,
        // The row a first press left behind, which is what the machine would
        // have in hand after a restart.
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(SUBSCRIBER, "incomplete", None)],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        // Not the address, and not a customer: the subscription this person
        // already has, read back.
        > @r#"ToSourceStripe { call_id: 2, action: LoadSubscription { user_id: 00000000-0000-0000-0000-000000000000, subscription_id: "sub_1" } }"#,

        < Event::FromSourceStripe {
            call_id: 2,
            event: EventSourceStripe::SubscriptionLoaded {
                user_id: SUBSCRIBER,
                subscription: src_stripe::StripeSubscription {
                    id: "sub_1".to_string(),
                    status: "incomplete".to_string(),
                    customer: "cus_1".to_string(),
                    cancel_at_period_end: false,
                    ended_at: None,
                    items: src_stripe::StripeSubscriptionItems {
                        data: vec![src_stripe::StripeSubscriptionItem {
                            current_period_start: 0,
                            current_period_end: PAID_UNTIL.as_secs() as i64,
                        }],
                    },
                    latest_invoice: Some(src_stripe::StripeExpandable::Expanded(
                        src_stripe::StripeInvoice {
                            id: "in_1".to_string(),
                            confirmation_secret: Some(
                                src_stripe::StripeConfirmationSecret {
                                    client_secret: "pi_1_secret".to_string(),
                                },
                            ),
                        },
                    )),
                },
            },
        },

        // What Stripe said is written like a webhook, and the same invoice's
        // secret is handed back once it has landed.
        > Action::ToStore {
            call_id: 3,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },
        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Subscribing { client_secret: "pi_1_secret", publishable_key: "pk_test_1" } }"#,
    );
}

/// A subscription that expired without ever being paid is replaced, against
/// the customer we already have.
///
/// The other half of coming back: 23 hours on, Stripe has given up on the
/// invoice and there is nothing left to confirm against. A new subscription is
/// right here — and a new *customer* is not, which is the second action that
/// does not come out.
#[test]
fn a_subscription_that_expired_unpaid_is_replaced() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(SUBSCRIBER, "incomplete", None)],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToSourceStripe {
            call_id: 2,
            action: ActionSourceStripe::LoadSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 2,
            event: EventSourceStripe::SubscriptionLoaded {
                user_id: SUBSCRIBER,
                subscription: src_stripe::StripeSubscription {
                    id: "sub_1".to_string(),
                    status: "incomplete_expired".to_string(),
                    customer: "cus_1".to_string(),
                    cancel_at_period_end: false,
                    ended_at: Some(T0.as_secs() as i64),
                    items: src_stripe::StripeSubscriptionItems {
                        data: vec![src_stripe::StripeSubscriptionItem {
                            current_period_start: 0,
                            current_period_end: PAID_UNTIL.as_secs() as i64,
                        }],
                    },
                    latest_invoice: None,
                },
            },
        },

        // What Stripe said is written like a webhook, and the customer the
        // dead subscription hung off is reused.
        > Action::ToStore {
            call_id: 3,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },
        > @r#"ToSourceStripe { call_id: 4, action: CreateSubscription { user_id: 00000000-0000-0000-0000-000000000000, customer_id: "cus_00000000-0000-0000-0000-000000000000" } }"#,

        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },
        < Event::FromSourceStripe {
            call_id: 4,
            event: EventSourceStripe::SubscriptionCreated {
                user_id: SUBSCRIBER,
                subscription_id: "sub_2".to_string(),
                status: src_stripe::StripeStatus::Incomplete,
                client_secret: Some("pi_2_secret".to_string()),
            },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::SaveCustomer { user_id: SUBSCRIBER, .. },
        },
        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Subscribing { client_secret: "pi_2_secret", publishable_key: "pk_test_1" } }"#,
    );
}

/// Somebody who already pays is told so, rather than sold a second
/// subscription.
///
/// The refusal that costs money if it is missing: two live subscriptions is
/// one card charged twice for one app. `past_due` and `unpaid` land here too —
/// Stripe is still issuing invoices against those, so they are not free to
/// replace either.
#[test]
fn somebody_who_already_pays_is_told_so() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToSourceStripe {
            call_id: 2,
            action: ActionSourceStripe::LoadSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 2,
            event: EventSourceStripe::SubscriptionLoaded {
                user_id: SUBSCRIBER,
                subscription: src_stripe::StripeSubscription {
                    id: "sub_1".to_string(),
                    status: "active".to_string(),
                    customer: "cus_1".to_string(),
                    cancel_at_period_end: false,
                    ended_at: None,
                    items: src_stripe::StripeSubscriptionItems {
                        data: vec![src_stripe::StripeSubscriptionItem {
                            current_period_start: 0,
                            current_period_end: PAID_UNTIL.as_secs() as i64,
                        }],
                    },
                    latest_invoice: None,
                },
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },
        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Subscribed }"#,
    );
}

/// A webhook that never arrived is repaired by the read. Somebody who paid
/// and was never let in presses Subscribe, Stripe says `active`, and that is
/// believed the way the webhook would have been: written, and the same
/// session is served.
#[test]
fn a_subscription_read_back_from_stripe_is_believed_like_a_webhook() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(SUBSCRIBER, "incomplete", None)],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToSourceStripe {
            call_id: 2,
            action: ActionSourceStripe::LoadSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 2,
            event: EventSourceStripe::SubscriptionLoaded {
                user_id: SUBSCRIBER,
                subscription: src_stripe::StripeSubscription {
                    id: "sub_1".to_string(),
                    status: "active".to_string(),
                    customer: "cus_1".to_string(),
                    cancel_at_period_end: false,
                    ended_at: None,
                    items: src_stripe::StripeSubscriptionItems {
                        data: vec![src_stripe::StripeSubscriptionItem {
                            current_period_start: 0,
                            current_period_end: PAID_UNTIL.as_secs() as i64,
                        }],
                    },
                    latest_invoice: None,
                },
            },
        },

        // Written before the route is answered, the same write a webhook
        // makes, so a restart reads the repair back.
        > Action::ToStore {
            call_id: 3,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },
        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Subscribed }"#,

        // And the session that was shut out is served.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 4,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 5, action: ActionStore::LoadItems { .. } },
    );
}

/// And the lockout itself, gone: a late delivery about a subscription that was
/// replaced is news about nothing.
///
/// The residue of #107. One subscription per person means the one that was
/// replaced is nobody's, so nothing it says reaches anybody's entitlement —
/// which matters because Stripe retries a delivery for days, and a retry can
/// land after the subscription that replaced it was paid for.
///
/// The write still goes out, and is meant to: it is keyed by the subscription
/// rather than the user, so it finds the row only if the row still names that
/// subscription — and this one names the subscription that replaced it. The
/// claim is the account screen afterwards, still saying what the person
/// actually bought.
#[test]
fn a_late_delivery_about_a_replaced_subscription_takes_nothing_away() {
    let mut state = state();

    let paid = stripe_subscription_event(
        "evt_paid",
        T0.as_secs(),
        "sub_2",
        "active",
        PAID_UNTIL.as_secs(),
    );
    let paid_signature = stripe_signed(&paid, T0);

    let expired = stripe_subscription_event(
        "evt_expired",
        T0.as_secs(),
        "sub_1",
        "incomplete_expired",
        PAID_UNTIL.as_secs(),
    );
    let expired_signature = stripe_signed(&expired, T0);

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(SUBSCRIBER, "incomplete", None)],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SubscribeRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToSourceStripe {
            call_id: 2,
            action: ActionSourceStripe::LoadSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 2,
            event: EventSourceStripe::SubscriptionLoaded {
                user_id: SUBSCRIBER,
                subscription: src_stripe::StripeSubscription {
                    id: "sub_1".to_string(),
                    status: "incomplete_expired".to_string(),
                    customer: "cus_1".to_string(),
                    cancel_at_period_end: false,
                    ended_at: Some(T0.as_secs() as i64),
                    items: src_stripe::StripeSubscriptionItems {
                        data: vec![src_stripe::StripeSubscriptionItem {
                            current_period_start: 0,
                            current_period_end: PAID_UNTIL.as_secs() as i64,
                        }],
                    },
                    latest_invoice: None,
                },
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },
        > Action::ToSourceStripe {
            call_id: 4,
            action: ActionSourceStripe::CreateSubscription { .. },
        },
        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },
        < Event::FromSourceStripe {
            call_id: 4,
            event: EventSourceStripe::SubscriptionCreated {
                user_id: SUBSCRIBER,
                subscription_id: "sub_2".to_string(),
                status: src_stripe::StripeStatus::Incomplete,
                client_secret: Some("pi_2_secret".to_string()),
            },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::SaveCustomer { .. },
        },
        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Subscribing { client_secret: "pi_2_secret", publishable_key: "pk_test_1" } }"#,

        // That one is paid for.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived {
                signature: paid_signature,
                body: paid,
            },
        },

        > @r#"ToRouter { request_id: 2, action: Accepted }"#,

        > Action::ToStore {
            call_id: 6,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },
        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },

        // And the delivery about the one it replaced arrives. Taken, because
        // the signature was ours and refusing it only asks Stripe to send it
        // again — and acted on no further.
        < Event::FromRouter {
            request_id: 3,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived {
                signature: expired_signature,
                body: expired,
            },
        },

        > @r#"ToRouter { request_id: 3, action: Accepted }"#,

        // Keyed by the subscription, so it matches no row: this user's names
        // `sub_2` now. Sent anyway, because a row can exist in the store that
        // this process has not got in hand.
        > Action::ToStore {
            call_id: 7,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },

        // Still paid up for the year they bought.
        < Event::FromRouter {
            request_id: 4,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BillingStatusRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 8,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 8,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToRouter { request_id: 4, action: BillingStatus { entitled: true, unpaid: None, good_until: Some(1818633600), cancel_at_period_end: false } }"#,

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },
    );
}

/// G, on the web's road: a session with no subscription asks for a Checkout
/// page, and gets the address of one after the customer it sells against is
/// made and written down.
///
/// The write lands before the route is answered, because the subscription the
/// page sells arrives by webhook and the webhook names only the customer — a
/// restart in between must still find whose customer it is.
#[test]
fn a_checkout_makes_a_customer_and_answers_with_stripes_page() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CheckoutRequested {
                token_sha256: hash_token("session-1"),
                origin: Some(WEB_ORIGIN.to_string()),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadUser { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::UserLoaded {
                email: Some("al@example.com".to_string()),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CreateCustomer {
                user_id: SUBSCRIBER,
                ..
            },
        },
        < Event::FromSourceStripe {
            call_id: 3,
            event: EventSourceStripe::CustomerCreated {
                user_id: SUBSCRIBER,
                customer_id: "cus_1".to_string(),
            },
        },

        // The customer is written down with no subscription against it, and
        // the page is asked for. The window is sent home to the origin the
        // request came from, and to no other.
        > Action::ToStore {
            call_id: 4,
            action: ActionStore::SaveCustomer { user_id: SUBSCRIBER, subscription_id: None, .. },
        },
        > @r#"ToSourceStripe { call_id: 5, action: OpenCheckout { user_id: 00000000-0000-0000-0000-000000000000, customer_id: "cus_1", success_url: "http://127.0.0.1:8080/paid.html", cancel_url: "http://127.0.0.1:8080/left.html" } }"#,

        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },

        < Event::FromSourceStripe {
            call_id: 5,
            event: EventSourceStripe::CheckoutOpened {
                user_id: SUBSCRIBER,
                url: "https://checkout.stripe.com/c/pay/cs_1".to_string(),
            },
        },

        > @r#"ToRouter { request_id: 1, action: CheckoutOpened { url: "https://checkout.stripe.com/c/pay/cs_1" } }"#,
    );
}

/// A checkout asked for from anywhere that is not the web app has no window
/// to send Stripe's page back to, so no page is opened. The phone sends no
/// `Origin`, and an origin the web app is not served from is a header
/// anybody can type.
#[test]
fn a_checkout_from_somewhere_that_is_not_the_web_app_is_refused() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CheckoutRequested {
                token_sha256: hash_token("session-1"),
                origin: None,
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToRouter { request_id: 1, action: ActionRouter::Rejected },
    );
}

/// A checkout pressed by somebody already paying sells nothing. The
/// subscription on file is read back first — which is the same read that
/// repairs a webhook that never arrived — and what Stripe says is written
/// before the route hears there is nothing to buy.
#[test]
fn a_checkout_for_somebody_already_paying_sells_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CheckoutRequested {
                token_sha256: hash_token("session-1"),
                origin: Some(WEB_ORIGIN.to_string()),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > @r#"ToSourceStripe { call_id: 2, action: LoadSubscription { user_id: 00000000-0000-0000-0000-000000000000, subscription_id: "sub_1" } }"#,

        < Event::FromSourceStripe {
            call_id: 2,
            event: EventSourceStripe::SubscriptionLoaded {
                user_id: SUBSCRIBER,
                subscription: src_stripe::StripeSubscription {
                    id: "sub_1".to_string(),
                    status: "active".to_string(),
                    customer: "cus_1".to_string(),
                    cancel_at_period_end: false,
                    ended_at: None,
                    items: src_stripe::StripeSubscriptionItems {
                        data: vec![src_stripe::StripeSubscriptionItem {
                            current_period_start: 0,
                            current_period_end: PAID_UNTIL.as_secs() as i64,
                        }],
                    },
                    latest_invoice: None,
                },
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },
        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > Action::ToRouter { request_id: 1, action: ActionRouter::Subscribed },
    );
}

/// A subscription born on Stripe's own Checkout page arrives as a webhook
/// naming a subscription this process has never heard of — and a customer it
/// has. The customer is how its user is found, the row is written the way a
/// subscribe's would have been, and the entitlement opens the app.
///
/// The machine starts from the store's rows rather than from the checkout
/// conversation, so this is also the restart story: a server that came up
/// between the page opening and the money being taken still attributes it.
#[test]
fn a_webhook_for_a_subscription_born_on_stripes_page_finds_its_user_by_customer() {
    let mut state = state();

    let paid = stripe_subscription_event(
        "evt_1",
        T0.as_secs() + 60,
        "sub_9",
        "active",
        PAID_UNTIL.as_secs(),
    );
    let paid_signature = stripe_signed(&paid, Timestamp::from_secs(T0.as_secs() + 60));

    eval!(&mut state,
        // The row the checkout left behind: a customer, and no subscription
        // against it yet.
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![src_postgres::StoredSubscription {
                    user_id: SUBSCRIBER,
                    provider: src_postgres::BillingProvider::Stripe,
                    customer_id: "cus_1".to_string(),
                    subscription_id: None,
                    status: "none".to_string(),
                    current_period_end: None,
                    cancel_at_period_end: false,
                    last_event_id: None,
                    last_event_at: None,
                }],
            },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::StripeWebhookReceived {
                signature: paid_signature,
                body: paid,
            },
        },

        > Action::ToRouter { request_id: 1, action: ActionRouter::Accepted },

        // Filed under the customer's user, then the event applied — the same
        // two writes a subscribe leaves, made from the webhook because the
        // webhook is the first news of this subscription.
        > Action::ToStore {
            call_id: 1,
            action: ActionStore::SaveCustomer {
                user_id: SUBSCRIBER,
                subscription_id: Some(_),
                ..
            },
        },
        > Action::ToStore {
            call_id: 2,
            action: ActionStore::ApplySubscriptionEvent { .. },
        },
        < Event::FromStore { call_id: 1, event: EventStore::WriteSuccess },
        < Event::FromStore { call_id: 2, event: EventStore::WriteSuccess },

        // And the person it names is served, which is the whole point.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) },
        },

        > Action::ToStore { call_id: 4, action: ActionStore::LoadItems { .. } },
    );
}
