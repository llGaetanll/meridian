//! Flow I of `docs/API_FLOW.md` — removing one bank — the way `docs/STYLE.md`
//! says: events in with `<`, actions out with `>`.
//!
//! The same two steps a deletion takes for every bank the account has, on the
//! one bank the caller names: stop the Item at Plaid, then forget what came
//! through it. In that order, because the access token is what the removal
//! needs and forgetting the Item takes the token with it.
//!
//! What differs from a deletion is what happens when Plaid refuses. There the
//! failure is logged and the rows go anyway — the alternative is an account
//! nobody can delete. Here the rows stay: the token is still the only thing
//! that can stop the Item being billed, and a caller told the bank is gone
//! would have no tap left to try again with.

mod fixtures;

use api_eval::eval;
use fixtures::SUBSCRIBER;
use fixtures::state_subscribed;

use server_main::*;

/// The happy path: the session, the tokens, Plaid, the rows, and only then
/// the answer.
#[test]
fn removing_a_bank_stops_it_at_plaid_before_it_forgets_the_rows() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BankRemoveRequested {
                token_sha256: hash_token("session-1"),
                item_id: "item-1".to_string(),
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

        // The tokens are the ownership check and what the removal needs in
        // one: a bank whose token is not among this caller's is not theirs.
        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadAccessTokens { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::AccessTokensLoaded {
                tokens: vec![
                    ("item-1".to_string(), "access-sandbox-1".to_string()),
                    ("item-2".to_string(), "access-sandbox-2".to_string()),
                ],
            },
        },

        // The named bank's token, and no other.
        > @r#"ToSourcePlaid { call_id: 3, action: RemoveItem { item_id: "item-1", access_token: "access-sandbox-1" } }"#,
        < Event::FromSourcePlaid {
            call_id: 3,
            event: EventSourcePlaid::ItemRemoved {
                item_id: "item-1".to_string(),
                removed: true,
            },
        },

        // Then the rows. The accounts, transactions and streams go with the
        // Item by the cascade the schema declares.
        > @r#"ToStore { call_id: 4, action: ForgetItem { item_id: "item-1" } }"#,
        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: BankRemoved }"#,
    );
}

/// A bank the caller does not hold is refused, and nothing is asked of Plaid.
/// No such bank and not yours are one answer, for the reason a repair's are:
/// telling them apart says which Items exist.
#[test]
fn a_bank_that_is_not_the_callers_is_refused_and_plaid_is_not_asked() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BankRemoveRequested {
                token_sha256: hash_token("session-1"),
                item_id: "item-nobody-has".to_string(),
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
            action: ActionStore::LoadAccessTokens { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::AccessTokensLoaded {
                tokens: vec![(
                    "item-1".to_string(),
                    "access-sandbox-1".to_string(),
                )],
            },
        },

        > @r#"ToRouter { request_id: 1, action: Rejected }"#,
    );
}

/// Plaid would not stop the Item. Nothing is forgotten, so the token that can
/// stop it being billed is still there and the tap can be made again.
#[test]
fn a_removal_plaid_refused_keeps_the_rows() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BankRemoveRequested {
                token_sha256: hash_token("session-1"),
                item_id: "item-1".to_string(),
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
            action: ActionStore::LoadAccessTokens { user_id: SUBSCRIBER },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::AccessTokensLoaded {
                tokens: vec![(
                    "item-1".to_string(),
                    "access-sandbox-1".to_string(),
                )],
            },
        },

        > Action::ToSourcePlaid {
            call_id: 3,
            action: ActionSourcePlaid::RemoveItem { .. },
        },
        < Event::FromSourcePlaid {
            call_id: 3,
            event: EventSourcePlaid::ItemRemoved {
                item_id: "item-1".to_string(),
                removed: false,
            },
        },

        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,
    );
}
