//! Flow A of `docs/API_FLOW.md` — signing in — tested the way
//! `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! The doc's A is two requests: `POST /auth/request` asks for a six-digit
//! code, `POST /auth/verify` trades the typed digits for a session. The first
//! test here is that pair walked end to end; the rest are the lettered
//! answers under them — 4c and every way of being 6b — one apiece.
//!
//! A's step 2 and 3 are Resend's, and they are the one part of the flow no
//! test here can reach: the machine emits
//! [`ActionEmail::SendCode`](server_main::ActionEmail) and hears nothing back,
//! because nothing is decided from what Resend answers. A4b, the rate limit,
//! is `state_limits.rs`.
//!
//! Every event is written out where it is sent rather than built by a helper,
//! so each test reads as the conversation it is. Three things recur: time
//! arrives as a `Tick` like any other fact from the edge, call ids are handed
//! out in order from 1 so an answer comes back on the one that asked, and a
//! route is released only once everything it started has been written — which
//! is what `EventStore::WriteSuccess` is doing on its own line.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use fixtures::T0;
use fixtures::state;

use server_main::*;

/// A. The whole of it: a code asked for, and the digits traded for a session.
///
/// The doc's steps 1, 4a, 5 and 6a, which is every entry in A that is not an
/// alternative answer or somebody else's request. Two round trips, and the
/// only thing carried between them is six digits the machine minted itself.
#[test]
fn a_first_run_asks_for_a_code_and_signs_in_with_it() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        // 1 and 2. The hash is stored and the code is sent, as two actions:
        // only one of them may ever hold the plaintext.
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeRequested {
                email: "gaetan@example.com".to_string(),
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::SaveCode { .. } },

        > @r#"ToEmail { action: SendCode { email: "gaetan@example.com", code: "470977" } }"#,

        // 4a. Taken, once the hash is actually down — a 204 sent before it
        // would be a code the next request cannot find.
        < Event::FromStore { call_id: 1, event: EventStore::WriteSuccess },

        > Action::ToRouter { request_id: 1, action: ActionRouter::Accepted },

        // 5. The digits that came out of the machine above, typed back in.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeSubmitted {
                email: "gaetan@example.com".to_string(),
                code: "470977".to_string(),
            },
        },

        > Action::ToStore { call_id: 2, action: ActionStore::LoadCode { .. } },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::CodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("470977"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::CreateSession { .. },
        },

        > Action::ToStore {
            call_id: 4,
            action: ActionStore::DeleteCode { .. },
        },

        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        // 6a. Only now: a token that reaches the phone before its session row
        // exists is a sign-in that 401s on its own first request.
        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },

        > Action::ToRouter { action: ActionRouter::SignedIn { .. }, .. },
    );
}

/// Something that is not an address is taken and nothing is sent.
///
/// Taken, because answering otherwise would tell whoever asked which strings
/// this server will write to — the same reason the route answers the same way
/// whether or not the address has an account. What the claim is about is the
/// two actions that do *not* come out: no hash is stored and no mail is sent.
///
/// It is decided here rather than in the handler, which is what makes it a
/// test at all: the check used to return 204 before any event existed.
#[test]
fn a_sign_in_asked_for_with_something_that_is_not_an_address_sends_nothing() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeRequested { email: "alex".to_string() },
        },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,
    );
}

/// The whole reason the generator is in here: the same seed produces the same
/// code, so an event log replays to the value it produced the first time. Two
/// seeds, two codes, both written down — a test that only said "they differ"
/// would pass on a generator that had stopped being a replay of anything.
#[test]
fn the_same_seed_produces_the_same_code() {
    let mut seven = state();

    eval!(&mut seven,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeRequested {
                email: "gaetan@example.com".to_string(),
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::SaveCode { .. } },

        > @r#"ToEmail { action: SendCode { email: "gaetan@example.com", code: "470977" } }"#,
    );

    let mut eight = state();

    eval!(&mut eight,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [8; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeRequested {
                email: "gaetan@example.com".to_string(),
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::SaveCode { .. } },

        > @r#"ToEmail { action: SendCode { email: "gaetan@example.com", code: "751695" } }"#,
    );
}

/// A4c. No entropy, no credentials. A predictable sign-in code is a way into
/// somebody's bank, so a machine that is up but not yet seeded refuses rather
/// than filling in anything derivable — and this is the test that says nothing
/// is stored and nothing is sent.
///
/// It answers rather than going quiet. A route parks until its own action
/// comes back, so a refusal with no action is a request that never returns.
#[test]
fn an_unseeded_machine_will_not_mint_a_code() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeRequested {
                email: "gaetan@example.com".to_string(),
            },
        },

        > Action::ToRouter { action: ActionRouter::Unavailable, .. },
    );
}

/// Spending it. `DeleteCode` is what the accepting turn emits; this is what
/// that means to somebody sending the same digits again.
#[test]
fn a_spent_code_signs_nobody_in_twice() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeSubmitted {
                email: "gaetan@example.com".to_string(),
                code: "123456".to_string(),
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadCode { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::CodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::CreateSession { .. },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::DeleteCode { .. },
        },

        < Event::FromStore { call_id: 2, event: EventStore::WriteSuccess },

        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > Action::ToRouter { action: ActionRouter::SignedIn { .. }, .. },

        // The same six digits, replayed. The delete landed, so the read this
        // one asks for finds nothing at all — and nothing is rejected like any
        // other wrong answer.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeSubmitted {
                email: "gaetan@example.com".to_string(),
                code: "123456".to_string(),
            },
        },

        > Action::ToStore { call_id: 4, action: ActionStore::LoadCode { .. } },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::CodeLoaded { stored: None },
        },

        > Action::ToRouter { action: ActionRouter::Rejected, .. },
    );
}

/// A6b. A wrong code costs a guess and nothing else — no session, and the code
/// is still there to be got right.
#[test]
fn a_wrong_code_only_counts_a_guess() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeSubmitted {
                email: "gaetan@example.com".to_string(),
                code: "654321".to_string(),
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadCode { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::CodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 0,
                }),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::RecordAttempt { .. },
        },
        < Event::FromStore { call_id: 2, event: EventStore::WriteSuccess },

        > Action::ToRouter { action: ActionRouter::Rejected, .. },
    );
}

/// An expired code counts a guess too. An attacker who could reset the cap by
/// waiting would have an unlimited number of tries at the next one.
#[test]
fn an_expired_code_is_rejected_and_still_counts() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeSubmitted {
                email: "gaetan@example.com".to_string(),
                code: "123456".to_string(),
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadCode { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::CodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs()),
                    attempts: 0,
                }),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::RecordAttempt { .. },
        },
        < Event::FromStore { call_id: 2, event: EventStore::WriteSuccess },

        > Action::ToRouter { action: ActionRouter::Rejected, .. },
    );
}

/// Out of guesses outranks a right answer, or the cap would be advice.
#[test]
fn an_exhausted_code_rejects_the_right_answer() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeSubmitted {
                email: "gaetan@example.com".to_string(),
                code: "123456".to_string(),
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadCode { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::CodeLoaded {
                stored: Some(StoredCode {
                    code_sha256: hash_code("123456"),
                    expires_at: Timestamp::from_secs(T0.as_secs() + 600),
                    attempts: 5,
                }),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::RecordAttempt { .. },
        },
        < Event::FromStore { call_id: 2, event: EventStore::WriteSuccess },

        > Action::ToRouter { action: ActionRouter::Rejected, .. },
    );
}

/// No code standing against the address. Rejected like any other, and
/// deliberately not distinguishable — the difference would say which addresses
/// have a sign-in in flight.
#[test]
fn an_address_with_no_code_is_rejected_the_same_way() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeSubmitted {
                email: "gaetan@example.com".to_string(),
                code: "123456".to_string(),
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadCode { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::CodeLoaded { stored: None },
        },

        > Action::ToRouter { action: ActionRouter::Rejected, .. },
    );
}

/// A6d. A store that will not answer is not a wrong code.
///
/// The read the machine asked for is the only way it can know, so there is
/// nothing to fall back on: refusing would be indistinguishable from a bad
/// code, and accepting would be worse. The route is told it cannot be
/// answered, which is the one thing that is true — and `/auth/verify` is the
/// one route that renders that as a 500 rather than swallowing it.
///
/// New with the read becoming an action. It used to be a 500 the handler
/// returned before any event existed, so nothing about it was replayable.
#[test]
fn a_store_that_will_not_answer_is_not_a_rejection() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::CodeSubmitted {
                email: "gaetan@example.com".to_string(),
                code: "123456".to_string(),
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadCode { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::ReadError {
                message: "connection refused".to_string(),
            },
        },

        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,
    );
}
