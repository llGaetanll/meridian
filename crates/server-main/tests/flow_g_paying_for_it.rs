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
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SubscribeRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToStore { action: ActionStore::LoadUser { call_id: 2, user_id: SUBSCRIBER } },

        < Event::FromStore { call_id: 2, event: EventStore::UserLoaded { email: Some("al@example.com".to_string()) } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CreateCustomer { call_id: 3, user_id: SUBSCRIBER, .. } },

        < Event::FromSourceStripe { event: EventSourceStripe::CustomerCreated { call_id: 3, user_id: SUBSCRIBER, customer_id: "cus_1".to_string() } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CreateSubscription { call_id: 4, user_id: SUBSCRIBER, .. } },

        // The row is written whatever happens next: the webhook that says this
        // was paid for names only the subscription, so without it there is
        // nothing to look the user up by.
        < Event::FromSourceStripe { event: EventSourceStripe::SubscriptionCreated { call_id: 4, user_id: SUBSCRIBER, subscription_id: "sub_1".to_string(), status: "incomplete".to_string(), client_secret: Some("pi_1_secret".to_string()) } },
        > Action::ToStore { action: ActionStore::SaveCustomer { call_id: 5, user_id: SUBSCRIBER, .. } },

        // Answered only once that write has landed. A route is released when
        // nothing it started is still out, so the row exists before the card
        // form does.
        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: Subscribing { client_secret: "pi_1_secret" } }"#,
    );
}

/// The route answering is not a payment. Nothing has been charged when the
/// card form opens, so the session is still refused everything.
#[test]
fn a_started_subscription_entitles_nobody() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SubscribeRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToStore { action: ActionStore::LoadUser { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::UserLoaded { email: Some("al@example.com".to_string()) } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CreateCustomer { call_id: 3, .. } },

        < Event::FromSourceStripe { event: EventSourceStripe::CustomerCreated { call_id: 3, user_id: SUBSCRIBER, customer_id: "cus_1".to_string() } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CreateSubscription { call_id: 4, .. } },

        < Event::FromSourceStripe { event: EventSourceStripe::SubscriptionCreated { call_id: 4, user_id: SUBSCRIBER, subscription_id: "sub_1".to_string(), status: "incomplete".to_string(), client_secret: Some("pi_1_secret".to_string()) } },
        > Action::ToStore { action: ActionStore::SaveCustomer { call_id: 5, .. } },

        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: Subscribing { client_secret: "pi_1_secret" } }"#,

        // The same session, asking for the thing it pays for.
        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 6, .. } },

        < Event::FromStore { call_id: 6, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > @r#"ToRouter { request_id: 2, action: PaymentRequired }"#,
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
        < Event::FromStore { call_id: 0, event: EventStore::SubscriptionsLoaded { subscriptions: vec![fixtures::subscription(SUBSCRIBER, "incomplete", None)] } },

        // Answered in the same turn the signature checks out, before the write
        // lands: Stripe retries anything it does not hear about.
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::StripeWebhookReceived { signature: signature.clone(), body: body.clone() } },
        > @r#"ToRouter { request_id: 1, action: Accepted }"#,
        > Action::ToStore { action: ActionStore::ApplySubscriptionEvent { call_id: 1, .. } },

        // And now the app opens.
        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 3, .. } },
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
        < Event::FromStore { call_id: 0, event: EventStore::SubscriptionsLoaded { subscriptions: vec![fixtures::subscription(SUBSCRIBER, "incomplete", None)] } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::StripeWebhookReceived { signature, body } },
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
        < Event::FromStore { call_id: 0, event: EventStore::SubscriptionsLoaded { subscriptions: vec![fixtures::subscription(SUBSCRIBER, "incomplete", None)] } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::StripeWebhookReceived { signature, body: tampered } },
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
        < Event::FromStore { call_id: 0, event: EventStore::SubscriptionsLoaded { subscriptions: vec![fixtures::subscription(SUBSCRIBER, "incomplete", None)] } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::StripeWebhookReceived { signature, body } },
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
                    stripe_webhook_secret: None,
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
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::StripeWebhookReceived { signature, body } },
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
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::StripeWebhookReceived { signature, body } },
        > @r#"ToRouter { request_id: 1, action: Accepted }"#,
        > Action::ToStore { action: ActionStore::ApplySubscriptionEvent { call_id: 1, .. } },
    );
}

/// An event about something that is not a subscription is taken and acted on
/// no further. Refusing it only asks Stripe to send it again.
#[test]
fn an_invoice_says_nothing_about_entitlement() {
    let mut state = state();

    let body = br#"{"id":"evt_1","type":"invoice.paid","created":1787097600,"data":{"object":{"object":"invoice","id":"in_1","customer":"cus_1"}}}"#.to_vec();
    let signature = stripe_signed(&body, T0);

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::StripeWebhookReceived { signature, body } },
        > @r#"ToRouter { request_id: 1, action: Accepted }"#,
    );
}

/// Cancelling does not close the app. The period paid for is what decides, and
/// `site/app/terms` promises exactly this.
#[test]
fn a_cancelled_subscription_still_opens_the_app_until_its_period_ends() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromStore { call_id: 0, event: EventStore::SubscriptionsLoaded { subscriptions: vec![fixtures::subscription(SUBSCRIBER, "canceled", Some(PAID_UNTIL))] } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },
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
        < Event::FromStore { call_id: 0, event: EventStore::SubscriptionsLoaded { subscriptions: vec![fixtures::subscription(SUBSCRIBER, "active", Some(ran_out))] } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > @r#"ToRouter { request_id: 1, action: PaymentRequired }"#,
    );
}

/// Entitlement lapses because the clock moved, and nothing sweeps it. The same
/// user, the same row, one tick either side of the period ending.
#[test]
fn a_period_ends_because_a_tick_says_so() {
    let mut state = state();

    let ends = Timestamp::from_secs(T0.as_secs() + 60);

    eval!(&mut state,
        < Event::FromStore { call_id: 0, event: EventStore::SubscriptionsLoaded { subscriptions: vec![fixtures::subscription(SUBSCRIBER, "active", Some(ends))] } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        // Finished, so the tick below has nothing of its own to give up on.
        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![] } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 3, .. } },

        < Event::FromStore { call_id: 3, event: EventStore::SnapshotLoaded { snapshot: server_wire::WireSnapshot::new(&[], &[], &[]) } },
        > Action::ToRouter { request_id: 1, action: ActionRouter::Serve { .. } },

        // A minute later, and nothing else has happened.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(ends.as_secs() + 1) } },

        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 4, .. } },

        < Event::FromStore { call_id: 4, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > @r#"ToRouter { request_id: 2, action: PaymentRequired }"#,
    );
}

/// The two routes that lead to paying are the two an unpaid session may use.
/// Refusing them would leave somebody signed in with no way to become a
/// customer, which is the one thing the gate must not do.
#[test]
fn an_unpaid_session_can_still_ask_where_it_stands() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::BillingStatusRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > @r#"ToRouter { request_id: 1, action: BillingStatus { entitled: false, status: None, good_until: None, cancel_at_period_end: false } }"#,
    );
}

/// And once they have paid, it says so.
#[test]
fn a_paid_session_is_told_what_it_has() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::BillingStatusRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > @r#"ToRouter { request_id: 1, action: BillingStatus { entitled: true, status: Some("active"), good_until: Some(1818633600), cancel_at_period_end: false } }"#,
    );
}

/// A token that names nobody is not a paywall. Told apart on purpose: one is a
/// sign-in screen and the other is a card form, and a client that conflated
/// them would sign somebody out for not having paid.
#[test]
fn a_token_that_names_nobody_is_unauthorized_rather_than_unpaid() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("stale") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: None } },
        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// Stripe refusing is not a subscription, and the caller is told so rather
/// than handed a secret that is not one.
#[test]
fn a_stripe_that_will_not_answer_is_not_a_subscription() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SubscribeRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToStore { action: ActionStore::LoadUser { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::UserLoaded { email: Some("al@example.com".to_string()) } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CreateCustomer { call_id: 3, .. } },

        < Event::FromSourceStripe { event: EventSourceStripe::StripeFailed { call_id: 3, user_id: SUBSCRIBER, message: "stripe said no".to_string() } },
        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,
    );
}
