//! Leaving — tested the way `docs/STYLE.md` says: events in with `<`, actions
//! out with `>`.
//!
//! Cancelling and deleting are one action, and that is a product decision with
//! a cost behind it: a Plaid Item is billed for as long as it *exists*, not for
//! as long as it is used, so an account that stops paying us and keeps its
//! banks is one we pay for every month for nothing. There is no paused state
//! because there is no way to afford one.
//!
//! The order is the whole design, and it is chosen by what is worst if the
//! process dies half way through:
//!
//! 1. **Stripe.** A card still being charged for an app that is gone is the
//!    failure a person notices.
//! 2. **The tokens**, read before anything is deleted — `items` goes with the
//!    user, and a token nobody holds is a bank that bills forever.
//! 3. **The banks**, at Plaid, which is the only way those bills stop.
//! 4. **The rows**, once no removal is still out.
//!
//! Every step past the first can be finished by hand from a log line. The
//! first cannot, which is why it goes first.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use fixtures::PAID_UNTIL;
use fixtures::SUBSCRIBER;
use fixtures::T0;
use fixtures::state;
use fixtures::state_subscribed;

use server_main::*;

fn hash_token(token: &str) -> Vec<u8> {
    use sha2::Digest;

    sha2::Sha256::digest(token.as_bytes()).to_vec()
}

/// H. The whole of it: a paying subscriber with one bank, gone.
#[test]
fn leaving_cancels_the_subscription_stops_the_bank_and_deletes_the_rows() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::DeleteRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        // Stripe first: a card that keeps being charged is the failure
        // somebody notices.
        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CancelSubscription { call_id: 2, user_id: SUBSCRIBER, .. } },

        // Then the tokens, while the rows they live in still exist.
        < Event::FromSourceStripe { event: EventSourceStripe::SubscriptionCancelled { call_id: 2, user_id: SUBSCRIBER, good_until: Some(1818633600) } },
        > Action::ToStore { action: ActionStore::LoadAccessTokens { call_id: 3, user_id: SUBSCRIBER } },

        // Then the bank, which is the only way its Plaid bill stops.
        < Event::FromStore { call_id: 3, event: EventStore::AccessTokensLoaded { tokens: vec![("item-1".to_string(), "access-sandbox-1".to_string())] } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::RemoveItem { call_id: 4, .. } },

        // And only then the rows.
        < Event::FromSourcePlaid { event: EventSourcePlaid::ItemRemoved { call_id: 4, item_id: "item-1".to_string(), removed: true } },
        > Action::ToStore { action: ActionStore::DeleteUser { call_id: 5, user_id: SUBSCRIBER } },

        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: Deleted }"#,
    );
}

/// Two banks, and the rows wait for both.
///
/// The claim is the `DeleteUser` that does *not* come out after the first
/// removal: deleting while a removal is still out is how an Item survives the
/// account and bills forever.
#[test]
fn every_bank_is_stopped_before_the_rows_go() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::DeleteRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CancelSubscription { call_id: 2, .. } },

        < Event::FromSourceStripe { event: EventSourceStripe::SubscriptionCancelled { call_id: 2, user_id: SUBSCRIBER, good_until: Some(1818633600) } },
        > Action::ToStore { action: ActionStore::LoadAccessTokens { call_id: 3, .. } },

        < Event::FromStore { call_id: 3, event: EventStore::AccessTokensLoaded { tokens: vec![("item-1".to_string(), "token-1".to_string()), ("item-2".to_string(), "token-2".to_string())] } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::RemoveItem { call_id: 4, .. } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::RemoveItem { call_id: 5, .. } },

        // One down. Nothing happens: the other is still out.
        < Event::FromSourcePlaid { event: EventSourcePlaid::ItemRemoved { call_id: 4, item_id: "item-1".to_string(), removed: true } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::ItemRemoved { call_id: 5, item_id: "item-2".to_string(), removed: true } },
        > Action::ToStore { action: ActionStore::DeleteUser { call_id: 6, .. } },

        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: Deleted }"#,
    );
}

/// A bank Plaid would not remove does not hold the deletion up.
///
/// It is logged, loudly, because it is a bill that keeps arriving — but an
/// account nobody can delete because of a bank they cannot see is worse, and
/// the deletion right in #16 does not have an exception for it.
#[test]
fn a_bank_that_will_not_stop_does_not_block_the_deletion() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::DeleteRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CancelSubscription { call_id: 2, .. } },

        < Event::FromSourceStripe { event: EventSourceStripe::SubscriptionCancelled { call_id: 2, user_id: SUBSCRIBER, good_until: Some(1818633600) } },
        > Action::ToStore { action: ActionStore::LoadAccessTokens { call_id: 3, .. } },

        < Event::FromStore { call_id: 3, event: EventStore::AccessTokensLoaded { tokens: vec![("item-1".to_string(), "token-1".to_string())] } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::RemoveItem { call_id: 4, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::ItemRemoved { call_id: 4, item_id: "item-1".to_string(), removed: false } },
        > Action::ToStore { action: ActionStore::DeleteUser { call_id: 5, .. } },

        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: Deleted }"#,
    );
}

/// An account with no bank skips Plaid entirely.
#[test]
fn an_account_with_no_bank_goes_straight_to_the_rows() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::DeleteRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CancelSubscription { call_id: 2, .. } },

        < Event::FromSourceStripe { event: EventSourceStripe::SubscriptionCancelled { call_id: 2, user_id: SUBSCRIBER, good_until: Some(1818633600) } },
        > Action::ToStore { action: ActionStore::LoadAccessTokens { call_id: 3, .. } },

        < Event::FromStore { call_id: 3, event: EventStore::AccessTokensLoaded { tokens: vec![] } },
        > Action::ToStore { action: ActionStore::DeleteUser { call_id: 4, .. } },

        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: Deleted }"#,
    );
}

/// **Somebody who never paid can still leave.**
///
/// There is nothing to cancel, so Stripe is skipped and the deletion starts at
/// the tokens. A deletion refused for want of a subscription is a deletion
/// right that does not exist — which App Store guideline 5.1.1(v) and the
/// right in #16 both require, whatever the billing state.
#[test]
fn somebody_who_never_paid_can_still_delete_their_account() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::DeleteRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        // No cancellation: there is nothing to cancel.
        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToStore { action: ActionStore::LoadAccessTokens { call_id: 2, user_id: SUBSCRIBER } },

        < Event::FromStore { call_id: 2, event: EventStore::AccessTokensLoaded { tokens: vec![] } },
        > Action::ToStore { action: ActionStore::DeleteUser { call_id: 3, .. } },

        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: Deleted }"#,
    );
}

/// And so can somebody whose subscription lapsed. The route is not gated on
/// entitlement, deliberately.
#[test]
fn somebody_whose_subscription_lapsed_can_still_delete_their_account() {
    let mut state = state();

    let ran_out = Timestamp::from_secs(T0.as_secs() - 1);

    eval!(&mut state,
        < Event::FromStore { call_id: 0, event: EventStore::SubscriptionsLoaded { subscriptions: vec![fixtures::subscription(SUBSCRIBER, "canceled", Some(ran_out))] } },

        // The app is shut to them.
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > @r#"ToRouter { request_id: 1, action: PaymentRequired }"#,

        // And the door out is still open. The subscription is still
        // cancelled: a period that has run out is not a subscription Stripe
        // has stopped renewing.
        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::DeleteRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CancelSubscription { call_id: 3, .. } },

        < Event::FromSourceStripe { event: EventSourceStripe::SubscriptionCancelled { call_id: 3, user_id: SUBSCRIBER, good_until: None } },
        > Action::ToStore { action: ActionStore::LoadAccessTokens { call_id: 4, .. } },
    );
}

/// A token that names nobody deletes nothing.
#[test]
fn a_token_that_names_nobody_deletes_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::DeleteRequested { token_sha256: hash_token("stale") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: None } },
        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// A Stripe that will not cancel stops the deletion, and says so.
///
/// Deliberately not "carry on anyway": deleting the rows while the
/// subscription still renews is the one outcome nobody can recover from
/// afterwards, because the row that says who was being charged is gone. The
/// caller is told to try again.
#[test]
fn a_stripe_that_will_not_cancel_deletes_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::DeleteRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CancelSubscription { call_id: 2, .. } },

        < Event::FromSourceStripe { event: EventSourceStripe::StripeFailed { call_id: 2, user_id: SUBSCRIBER, message: "stripe said no".to_string() } },
        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,
    );
}

/// The app closes for the deleted account, and a webhook about it afterwards
/// finds nobody.
///
/// Both matter: entitlement held in memory would otherwise outlive the rows,
/// and a `customer.subscription.deleted` arriving a moment later must not name
/// a user id that no longer means anything.
#[test]
fn a_deleted_account_is_forgotten_whole() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::DeleteRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToSourceStripe { action: ActionSourceStripe::CancelSubscription { call_id: 2, .. } },

        < Event::FromSourceStripe { event: EventSourceStripe::SubscriptionCancelled { call_id: 2, user_id: SUBSCRIBER, good_until: Some(1818633600) } },
        > Action::ToStore { action: ActionStore::LoadAccessTokens { call_id: 3, .. } },

        < Event::FromStore { call_id: 3, event: EventStore::AccessTokensLoaded { tokens: vec![] } },
        > Action::ToStore { action: ActionStore::DeleteUser { call_id: 4, .. } },

        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: Deleted }"#,

        // The session is gone with the rows, but even if it were not, the
        // entitlement it relied on is.
        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 5, .. } },

        < Event::FromStore { call_id: 5, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > @r#"ToRouter { request_id: 2, action: PaymentRequired }"#,
    );
}

/// Cancelling on its own — through Stripe's own dashboard, say — leaves the
/// account working to the end of the period paid for.
///
/// The pairing with deletion is *our* door out, not Stripe's only one, and the
/// terms promise the period either way.
#[test]
fn a_cancellation_that_did_not_come_through_us_still_serves_the_period() {
    let mut state = state();

    let body = fixtures::stripe_subscription_event(
        "evt_1",
        T0.as_secs(),
        "sub_1",
        "canceled",
        PAID_UNTIL.as_secs(),
    );

    let signature = fixtures::stripe_signed(&body, T0);

    eval!(&mut state,
        < Event::FromStore { call_id: 0, event: EventStore::SubscriptionsLoaded { subscriptions: vec![fixtures::subscription(SUBSCRIBER, "active", Some(PAID_UNTIL))] } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::StripeWebhookReceived { signature, body } },
        > @r#"ToRouter { request_id: 1, action: Accepted }"#,
        > Action::ToStore { action: ActionStore::ApplySubscriptionEvent { call_id: 1, .. } },

        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::SessionResolved { user_id: Some(SUBSCRIBER) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 3, .. } },
    );
}
