//! What every route in `docs/API_FLOW.md` answers when a caller asks too
//! often — A4b, B4c, C1 6d, C2 8e, D4d and E2c, which are all one bucket
//! apiece — tested the way `docs/STYLE.md` says: events in with `<`, actions
//! out with `>`.
//!
//! Two limits live here and they are different things. Plaid's is per Item and
//! per endpoint, and getting it wrong costs money: a call Plaid rejects still
//! counts against the budget, so a burst has to be refused here rather than
//! sent and bounced. Ours is per caller, and on `/auth/request` per *inbox*
//! as well, because a botnet has an address each and one victim.
//!
//! The webhook is the one route with no bucket at all. What protects it is
//! `state_webhook_trust.rs`.
//!
//! Filling a window is what the helpers at the top do, and they are helpers
//! *around* a test: which turn spends which is never the claim here, only that
//! the next one is refused.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use fixtures::T0;
use fixtures::signed;
use fixtures::state;
use fixtures::webhook_jwk;

use server_main::*;

/// Drive one whole `/snapshot` request to its answer, answering the reads it
/// asks for along the way.
fn spend(state: &mut StateServer, request_id: u64, items: &[ItemFacts]) {
    let mut actions = Vec::new();

    state.handle_events(
        &[Event::FromRouter {
            request_id,
            client: Client {
                ip: ClientIp::new("203.0.113.7"),
                user: None,
            },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("session-1"),
            },
        }],
        &mut actions,
    );

    loop {
        let answers: Vec<Event> = actions
            .iter()
            .filter_map(|action| read_back(action, items))
            .collect();

        if answers.is_empty() {
            return;
        }

        actions.clear();
        state.handle_events(&answers, &mut actions);
    }
}

/// The event that answers one store read, if that is what this action is.
fn read_back(action: &Action, items: &[ItemFacts]) -> Option<Event> {
    let Action::ToStore { action } = action else {
        return None;
    };

    let event = match action {
        ActionStore::LoadSession { .. } => EventStore::SessionResolved {
            user_id: Some(uuid::Uuid::nil()),
        },

        ActionStore::LoadItems { .. } => EventStore::ItemsLoaded {
            items: items.to_vec(),
        },

        ActionStore::LoadSnapshot { .. } => EventStore::SnapshotLoaded {
            snapshot: WireSnapshot::default(),
        },

        _ => return None,
    };

    Some(Event::FromStore {
        call_id: action.call_id(),
        event,
    })
}

/// Ask for a sign-in code once, and drop what comes of it.
fn ask_for_a_code(state: &mut StateServer, request_id: u64, ip: &str, email: &str) {
    state.handle_events(
        &[Event::FromRouter {
            request_id,
            client: Client {
                ip: ClientIp::new(ip),
                user: None,
            },
            event: EventRouter::CodeRequested {
                email: email.to_string(),
            },
        }],
        &mut Vec::new(),
    );
}

/// One whole webhook, driven to its answer, fetching the key if it is the
/// first.
fn deliver(state: &mut StateServer, request_id: u64, body: &str) {
    let mut actions = Vec::new();
    let (jwt, body) = signed(body, T0.as_secs());

    state.handle_events(
        &[Event::FromRouter {
            request_id,
            client: Client {
                ip: ClientIp::new("203.0.113.7"),
                user: None,
            },
            event: EventRouter::WebhookReceived { jwt, body },
        }],
        &mut actions,
    );

    let keys: Vec<Event> = actions
        .iter()
        .filter_map(|action| match action {
            Action::ToSource {
                action: ActionSource::FetchWebhookKey { call_id, kid },
            } => Some(Event::FromSource {
                event: EventSource::WebhookKeyLoaded {
                    call_id: *call_id,
                    kid: kid.clone(),
                    key: Some(webhook_jwk()),
                },
            }),

            _ => None,
        })
        .collect();

    if !keys.is_empty() {
        state.handle_events(&keys, &mut Vec::new());
    }
}

/// Plaid meters per endpoint, and a rejected call still counts against the
/// budget — so a burst has to be refused here rather than sent and bounced.
///
/// `BalanceGet` allows ten a minute and each refresh is its own route, so the
/// budget runs out in front of you: ten fetches, and then the eleventh is
/// answered from the store instead of asking Plaid again. The bank is stale
/// every time, so nothing but the limit can be what stops it.
#[test]
fn a_burst_of_refreshes_is_rate_limited() {
    let mut state = state();

    let stale = [ItemFacts {
        item_id: "item-1".to_string(),
        cursor: Some("cursor-1".to_string()),
        synced_at: Some(Timestamp::from_secs(T0.as_secs() - FRESH_FOR_SECS)),
        login_required: false,
    }];

    for request_id in 1..=10 {
        spend(&mut state, request_id, &stale);
    }

    eval!(&mut state,
        < Event::FromRouter { request_id: 11, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 31, .. } },

        < Event::FromStore { call_id: 31, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 32, .. } },

        // Stale, and asked for anyway — and no `FetchPlaid` comes out. Served
        // what the store already holds instead.
        < Event::FromStore { call_id: 32, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(Timestamp::from_secs(T0.as_secs() - FRESH_FOR_SECS)), login_required: false }] } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 33, .. } },

        < Event::FromStore { call_id: 33, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// The budget is per Item, so a second bank is not refused because the first
/// one spent its allowance.
#[test]
fn the_rate_limit_is_per_bank() {
    let mut state = state();

    let stale = [ItemFacts {
        item_id: "item-1".to_string(),
        cursor: Some("cursor-1".to_string()),
        synced_at: Some(Timestamp::from_secs(T0.as_secs() - FRESH_FOR_SECS)),
        login_required: false,
    }];

    for request_id in 1..=10 {
        spend(&mut state, request_id, &stale);
    }

    eval!(&mut state,
        < Event::FromRouter { request_id: 11, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 31, .. } },

        < Event::FromStore { call_id: 31, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 32, .. } },

        < Event::FromStore { call_id: 32, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-2".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },
    );
}

/// E2c. The point of the whole thing: a caller past its budget is answered and
/// nothing else happens — no fetch, no store write, no email. Refused on the
/// turn the session names them, and nothing is read for it: a request that is
/// not being served costs one lookup and a 429.
#[test]
fn a_caller_past_its_budget_is_refused() {
    let mut state = state();

    // Twenty a minute on `/snapshot`, spent.
    for request_id in 1..=20 {
        spend(&mut state, request_id, &[]);
    }

    eval!(&mut state,
        < Event::FromRouter { request_id: 21, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 61, .. } },

        < Event::FromStore { call_id: 61, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > @r#"ToRouter { request_id: 21, action: TooMany }"#,
    );
}

/// The budget comes back on its own, without anything having to reset it.
/// Time is the only thing that opens it, and time is an event.
#[test]
fn the_budget_returns_when_the_window_passes() {
    let mut state = state();

    for request_id in 1..=20 {
        spend(&mut state, request_id, &[]);
    }

    eval!(&mut state,
        < Event::FromRouter { request_id: 21, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 61, .. } },

        < Event::FromStore { call_id: 61, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToRouter { action: ActionRouter::TooMany, .. },

        // A minute passes. Nothing else happens in it.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + 61) } },

        < Event::FromRouter { request_id: 22, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 62, .. } },

        < Event::FromStore { call_id: 62, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 63, .. } },

        < Event::FromStore { call_id: 63, event: EventStore::ItemsLoaded { items: vec![] } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 64, .. } },

        < Event::FromStore { call_id: 64, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// A4b, and the refusal is the *only* thing that comes out: no `SaveCode`, and
/// above all no `SendCode` — the mailbomb this limit exists to stop. The
/// machine is seeded, so nothing but the limit can be what stops it.
#[test]
fn a_refused_request_starts_nothing() {
    let mut state = state();

    state.handle_events(
        &[Event::FromProcess {
            event: EventProcess::Seeded { seed: [7; 32] },
        }],
        &mut Vec::new(),
    );

    for request_id in 1..=5 {
        ask_for_a_code(&mut state, request_id, "203.0.113.7", "al@example.com");
    }

    eval!(&mut state,
        < Event::FromRouter { request_id: 6, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::CodeRequested { email: "al@example.com".to_string() } },
        > @r#"ToRouter { request_id: 6, action: TooMany }"#,
    );
}

/// One caller running out must not touch another's budget.
#[test]
fn one_caller_over_its_budget_does_not_refuse_the_next() {
    let mut state = state();

    state.handle_events(
        &[Event::FromProcess {
            event: EventProcess::Seeded { seed: [7; 32] },
        }],
        &mut Vec::new(),
    );

    for request_id in 1..=5 {
        ask_for_a_code(&mut state, request_id, "203.0.113.7", "al@example.com");
    }

    eval!(&mut state,
        < Event::FromRouter { request_id: 6, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::CodeRequested { email: "al@example.com".to_string() } },
        > Action::ToRouter { action: ActionRouter::TooMany, .. },

        // A different address asking for a different inbox goes through. Both
        // have to differ: the same inbox from a new address is the mailbomb
        // the test below is about. The `Accepted` that follows is held until
        // the write lands and belongs to `flow_a_signing_in.rs`.
        < Event::FromRouter { request_id: 7, client: Client { ip: ClientIp::new("198.51.100.4"), user: None }, event: EventRouter::CodeRequested { email: "sam@example.com".to_string() } },
        > Action::ToStore { action: ActionStore::SaveCode { .. } },
        > Action::ToEmail { action: ActionEmail::SendCode { .. } },
    );
}

/// The other half of #23's limit on this route. A botnet has an address each
/// and one victim, so every address window has room while one person takes
/// every message — which is why what is metered is the mail, not only the
/// caller.
#[test]
fn a_botnet_cannot_mailbomb_one_inbox() {
    let mut state = state();

    state.handle_events(
        &[Event::FromProcess {
            event: EventProcess::Seeded { seed: [7; 32] },
        }],
        &mut Vec::new(),
    );

    // Five codes, each asked for from an address of its own, so no address
    // window is anywhere near its limit.
    for request_id in 1..=5 {
        ask_for_a_code(
            &mut state,
            request_id,
            &format!("203.0.113.{request_id}"),
            "al@example.com",
        );
    }

    eval!(&mut state,
        // A sixth machine, and a tag on the address to go with it. Neither
        // buys another message: it is one inbox.
        < Event::FromRouter { request_id: 6, client: Client { ip: ClientIp::new("198.51.100.4"), user: None }, event: EventRouter::CodeRequested { email: "al+signup@example.com".to_string() } },
        > @r#"ToRouter { request_id: 6, action: TooMany }"#,
    );
}

/// A6c. `/auth/verify` has a bucket of its own — twenty a minute — so a caller
/// working through guesses runs out of the window long before they run out of
/// six-digit codes. The code's own five-attempt cap is a second wall behind
/// this one, and `flow_a_signing_in.rs` is where that is pinned.
#[test]
fn a_caller_working_through_codes_runs_out_of_tries() {
    let mut state = state();

    state.handle_events(
        &[Event::FromProcess {
            event: EventProcess::Seeded { seed: [7; 32] },
        }],
        &mut Vec::new(),
    );

    for request_id in 1..=20 {
        state.handle_events(
            &[Event::FromRouter {
                request_id,
                client: Client {
                    ip: ClientIp::new("203.0.113.7"),
                    user: None,
                },
                event: EventRouter::CodeSubmitted {
                    email: "al@example.com".to_string(),
                    code: "000000".to_string(),
                },
            }],
            &mut Vec::new(),
        );
    }

    eval!(&mut state,
        // No `LoadCode`: refused before the store is asked anything at all.
        < Event::FromRouter { request_id: 21, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::CodeSubmitted { email: "al@example.com".to_string(), code: "000000".to_string() } },
        > @r#"ToRouter { request_id: 21, action: TooMany }"#,
    );
}

/// Plaid calls us from addresses we do not choose, so the webhook is the one
/// route with no bucket. A hundred of them in a turn are all acted on.
#[test]
fn webhooks_are_not_rate_limited() {
    let mut state = state();

    for request_id in 1..=100 {
        deliver(
            &mut state,
            request_id,
            r#"{"webhook_type":"ITEM","webhook_code":"NEW_ACCOUNTS_AVAILABLE"}"#,
        );
    }

    eval!(&mut state,
        < Event::FromRouter { request_id: 101, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"ITEM\",\"webhook_code\":\"NEW_ACCOUNTS_AVAILABLE\"}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"ITEM\",\"webhook_code\":\"NEW_ACCOUNTS_AVAILABLE\"}", T0.as_secs()).1 } },
        > @r#"ToRouter { request_id: 101, action: Accepted }"#,
    );
}
