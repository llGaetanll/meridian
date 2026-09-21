//! Leaving — tested the way `docs/STYLE.md` says: events in with `<`, actions
//! out with `>`.
//!
//! Leaving is two requests. `POST /account/delete/request` emails the
//! account's address a six-digit code, and `POST /account/delete` trades the
//! typed digits for the deletion — so a stolen session cannot destroy an
//! account whose inbox it cannot read.
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

/// H. The whole of it: the code asked for and emailed, then the digits traded
/// for the deletion — a paying subscriber with one bank, gone.
#[test]
fn leaving_asks_the_inbox_first_and_the_code_deletes_everything() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        // The code is asked for. The address it goes to is the store's to
        // say, so the store is read; the hash is stored and the code is sent
        // as two actions, because only one of them may hold the plaintext.
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteCodeRequested {
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
                email: Some("gaetan@example.com".to_string()),
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::SaveDeleteCode { user_id: SUBSCRIBER, .. },
        },

        > @r#"ToEmail { action: SendDeleteCode { email: "gaetan@example.com", code: "470977" } }"#,

        // Taken, once the hash is actually down — a 204 sent before it would
        // be a code the next request cannot find.
        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,

        // The digits that came out of the machine above, typed back in.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "470977".to_string(),
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

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::LoadDeleteCode { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 5,
            event: EventStore::DeleteCodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("470977"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        // Stripe first: a card that keeps being charged is the failure
        // somebody notices.
        > Action::ToSourceStripe {
            call_id: 6,
            action: ActionSourceStripe::CancelSubscription {
                user_id: SUBSCRIBER,
                ..
            },
        },
        < Event::FromSourceStripe {
            call_id: 6,
            event: EventSourceStripe::SubscriptionCancelled {
                user_id: SUBSCRIBER,
                good_until: Some(1818633600),
            },
        },

        // Then the tokens, while the rows they live in still exist.
        > Action::ToStore {
            call_id: 7,
            action: ActionStore::LoadAccessTokens { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 7,
            event: EventStore::AccessTokensLoaded {
                tokens: vec![(
                    "item-1".to_string(),
                    "access-sandbox-1".to_string(),
                )],
            },
        },

        // Then the bank, which is the only way its Plaid bill stops.
        > Action::ToSourcePlaid {
            call_id: 8,
            action: ActionSourcePlaid::RemoveItem { .. },
        },
        < Event::FromSourcePlaid {
            call_id: 8,
            event: EventSourcePlaid::ItemRemoved {
                item_id: "item-1".to_string(),
                removed: true,
            },
        },

        // And only then the rows. The code row is not spent by an action of
        // its own: it goes with the account.
        > Action::ToStore {
            call_id: 9,
            action: ActionStore::DeleteUser { user_id: SUBSCRIBER },
        },
        < Event::FromStore { call_id: 9, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 2, action: Deleted }"#,
    );
}

/// A deletion code asked for twice at almost the same moment is emailed once
/// — the twin `flow_a_signing_in.rs` pins for sign-in codes, on the other
/// route that sends mail. The session and the address are still read, because
/// whose inbox it is has to be settled before anything can be absorbed; what
/// must not come out twice is the save and the mail.
#[test]
fn a_deletion_code_asked_for_twice_at_once_is_emailed_once() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteCodeRequested {
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
                email: Some("gaetan@example.com".to_string()),
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::SaveDeleteCode { user_id: SUBSCRIBER, .. },
        },

        > Action::ToEmail { action: ActionEmail::SendDeleteCode { .. } },

        // The twin arrives before the first save has landed.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteCodeRequested {
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

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::LoadUser { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 5,
            event: EventStore::UserLoaded {
                email: Some("gaetan@example.com".to_string()),
            },
        },

        > Action::ToRouter { request_id: 2, action: ActionRouter::Accepted },

        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > Action::ToRouter { request_id: 1, action: ActionRouter::Accepted },
    );
}

/// And once the cooldown has passed, asking again is served whole — the same
/// boundary `flow_a_signing_in.rs` pins, on this route.
#[test]
fn a_deletion_code_asked_for_after_the_cooldown_is_emailed_again() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteCodeRequested {
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
                email: Some("gaetan@example.com".to_string()),
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::SaveDeleteCode { user_id: SUBSCRIBER, .. },
        },

        > Action::ToEmail { action: ActionEmail::SendDeleteCode { .. } },

        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > Action::ToRouter { request_id: 1, action: ActionRouter::Accepted },

        // The cooldown runs out, to the second.
        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(T0.as_secs() + CODE_COOLDOWN_SECS),
            },
        },

        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteCodeRequested {
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

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::LoadUser { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 5,
            event: EventStore::UserLoaded {
                email: Some("gaetan@example.com".to_string()),
            },
        },

        > Action::ToStore {
            call_id: 6,
            action: ActionStore::SaveDeleteCode { user_id: SUBSCRIBER, .. },
        },

        > @r#"ToEmail { action: SendDeleteCode { email: "gaetan@example.com", code: "257670" } }"#,

        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },

        > Action::ToRouter { request_id: 2, action: ActionRouter::Accepted },
    );
}

/// A wrong code deletes nothing, and spends a guess.
///
/// Counted in the store rather than waved through, because the cap is what
/// stops a stolen code being brute-forced in the ten minutes it lives.
#[test]
fn a_wrong_code_deletes_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "000000".to_string(),
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
            action: ActionStore::LoadDeleteCode { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::DeleteCodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::RecordDeleteAttempt { user_id: SUBSCRIBER },
        },
        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Rejected }"#,
    );
}

/// A deletion nobody asked a code for is refused: the token alone is not
/// enough, which is the whole reason the code exists.
#[test]
fn a_deletion_with_no_code_standing_is_refused() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "123456".to_string(),
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
            action: ActionStore::LoadDeleteCode { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::DeleteCodeLoaded { stored: None },
        },

        > @r#"ToRouter { request_id: 1, action: Rejected }"#,
    );
}

/// A right code typed too late is refused like a wrong one, and counted.
#[test]
fn an_expired_code_deletes_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "123456".to_string(),
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
            action: ActionStore::LoadDeleteCode { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::DeleteCodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: T0,
                    attempts: 0,
                }),
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::RecordDeleteAttempt { user_id: SUBSCRIBER },
        },
        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Rejected }"#,
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
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "123456".to_string(),
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
            action: ActionStore::LoadDeleteCode { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::DeleteCodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CancelSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 3,
            event: EventSourceStripe::SubscriptionCancelled {
                user_id: SUBSCRIBER,
                good_until: Some(1818633600),
            },
        },

        > Action::ToStore {
            call_id: 4,
            action: ActionStore::LoadAccessTokens { .. },
        },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::AccessTokensLoaded {
                tokens: vec![
                    ("item-1".to_string(), "token-1".to_string()),
                    ("item-2".to_string(), "token-2".to_string()),
                ],
            },
        },

        > Action::ToSourcePlaid {
            call_id: 5,
            action: ActionSourcePlaid::RemoveItem { .. },
        },

        > Action::ToSourcePlaid {
            call_id: 6,
            action: ActionSourcePlaid::RemoveItem { .. },
        },

        // One down. Nothing happens: the other is still out.
        < Event::FromSourcePlaid {
            call_id: 5,
            event: EventSourcePlaid::ItemRemoved {
                item_id: "item-1".to_string(),
                removed: true,
            },
        },

        < Event::FromSourcePlaid {
            call_id: 6,
            event: EventSourcePlaid::ItemRemoved {
                item_id: "item-2".to_string(),
                removed: true,
            },
        },

        > Action::ToStore {
            call_id: 7,
            action: ActionStore::DeleteUser { .. },
        },
        < Event::FromStore { call_id: 7, event: EventStore::WriteSuccess },

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
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "123456".to_string(),
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
            action: ActionStore::LoadDeleteCode { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::DeleteCodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CancelSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 3,
            event: EventSourceStripe::SubscriptionCancelled {
                user_id: SUBSCRIBER,
                good_until: Some(1818633600),
            },
        },

        > Action::ToStore {
            call_id: 4,
            action: ActionStore::LoadAccessTokens { .. },
        },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::AccessTokensLoaded {
                tokens: vec![("item-1".to_string(), "token-1".to_string())],
            },
        },

        > Action::ToSourcePlaid {
            call_id: 5,
            action: ActionSourcePlaid::RemoveItem { .. },
        },
        < Event::FromSourcePlaid {
            call_id: 5,
            event: EventSourcePlaid::ItemRemoved {
                item_id: "item-1".to_string(),
                removed: false,
            },
        },

        > Action::ToStore {
            call_id: 6,
            action: ActionStore::DeleteUser { .. },
        },
        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Deleted }"#,
    );
}

/// An account with no bank skips Plaid entirely.
#[test]
fn an_account_with_no_bank_goes_straight_to_the_rows() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "123456".to_string(),
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
            action: ActionStore::LoadDeleteCode { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::DeleteCodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CancelSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 3,
            event: EventSourceStripe::SubscriptionCancelled {
                user_id: SUBSCRIBER,
                good_until: Some(1818633600),
            },
        },

        > Action::ToStore {
            call_id: 4,
            action: ActionStore::LoadAccessTokens { .. },
        },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::AccessTokensLoaded { tokens: vec![] },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::DeleteUser { .. },
        },
        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },

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
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "123456".to_string(),
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
            action: ActionStore::LoadDeleteCode { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::DeleteCodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        // No cancellation: there is nothing to cancel.
        > Action::ToStore {
            call_id: 3,
            action: ActionStore::LoadAccessTokens { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::AccessTokensLoaded { tokens: vec![] },
        },

        > Action::ToStore {
            call_id: 4,
            action: ActionStore::DeleteUser { .. },
        },
        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Deleted }"#,
    );
}

/// And so can somebody whose subscription lapsed. Neither route is gated on
/// entitlement, deliberately.
#[test]
fn somebody_whose_subscription_lapsed_can_still_delete_their_account() {
    let mut state = state();

    let ran_out = Timestamp::from_secs(T0.as_secs() - 1);

    eval!(&mut state,
        < Event::FromStore {
            call_id: 0,
            event: EventStore::SubscriptionsLoaded {
                subscriptions: vec![fixtures::subscription(
                    SUBSCRIBER,
                    "canceled",
                    Some(ran_out),
                )],
            },
        },

        // The app is shut to them.
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

        > @r#"ToRouter { request_id: 1, action: PaymentRequired { unpaid: WireUnpaid { reason: "ended", ran_out: Some(1787097599) } } }"#,

        // And the door out is still open. The subscription is still
        // cancelled: a period that has run out is not a subscription Stripe
        // has stopped renewing.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "123456".to_string(),
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

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::LoadDeleteCode { .. },
        },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::DeleteCodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        > Action::ToSourceStripe {
            call_id: 4,
            action: ActionSourceStripe::CancelSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 4,
            event: EventSourceStripe::SubscriptionCancelled {
                user_id: SUBSCRIBER,
                good_until: None,
            },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::LoadAccessTokens { .. },
        },
    );
}

/// A token that names nobody deletes nothing, and is not told whether a code
/// stands.
#[test]
fn a_token_that_names_nobody_deletes_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("stale"),
                code: "123456".to_string(),
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

/// A Stripe that will not cancel stops the deletion, and says so.
///
/// Deliberately not "carry on anyway": deleting the rows while the
/// subscription still renews is the one outcome nobody can recover from
/// afterwards, because the row that says who was being charged is gone. The
/// caller is told to try again — and the code row still stands for the retry,
/// which is why an accepted code is not spent by an action of its own.
#[test]
fn a_stripe_that_will_not_cancel_deletes_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "123456".to_string(),
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
            action: ActionStore::LoadDeleteCode { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::DeleteCodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CancelSubscription { .. },
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
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::DeleteRequested {
                token_sha256: hash_token("session-1"),
                code: "123456".to_string(),
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
            action: ActionStore::LoadDeleteCode { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::DeleteCodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        > Action::ToSourceStripe {
            call_id: 3,
            action: ActionSourceStripe::CancelSubscription { .. },
        },
        < Event::FromSourceStripe {
            call_id: 3,
            event: EventSourceStripe::SubscriptionCancelled {
                user_id: SUBSCRIBER,
                good_until: Some(1818633600),
            },
        },

        > Action::ToStore {
            call_id: 4,
            action: ActionStore::LoadAccessTokens { .. },
        },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::AccessTokensLoaded { tokens: vec![] },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::DeleteUser { .. },
        },
        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Deleted }"#,

        // The session is gone with the rows, but even if it were not, the
        // entitlement it relied on is.
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

        > Action::ToStore { call_id: 3, action: ActionStore::LoadItems { .. } },
    );
}
