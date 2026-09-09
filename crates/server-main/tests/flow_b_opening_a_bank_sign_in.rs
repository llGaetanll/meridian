//! Flow B of `docs/API_FLOW.md` — opening a bank sign-in — tested the way
//! `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! B is one request, `POST /link/start`, and the Link session it opens at
//! Plaid. Steps 5 and 6 are the sheet talking to Plaid with nothing of ours in
//! the exchange, so the machine's half of B ends at 4.
//!
//! B4c, the rate limit, is `state_limits.rs`. What the sheet closing leads to
//! is C, in `flow_c1_collecting_by_polling.rs` and
//! `flow_c2_collecting_by_webhook.rs`.

mod fixtures;

use api_eval::eval;
use fixtures::state;

use server_main::*;

/// B. A session opened and its page handed over — steps 1 through 4a.
///
/// The session is written down *before* the URL goes out. The other order
/// leaves a window in which a session could be finished that we have no record
/// of asking for, and the `user_id` on that row is what stops one account
/// collecting a bank another account connected.
#[test]
fn connecting_a_bank_opens_a_session_and_hands_over_its_page() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        // 2. Nothing is read for it: the id the session resolved to is the
        // whole of what opening a Link session needs.
        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::OpenLink { call_id: 2, .. } },

        // 3 and 4a, in that order and never the other.
        < Event::FromSourcePlaid { event: EventSourcePlaid::LinkOpened { call_id: 2, user_id: uuid::Uuid::nil(), link_token: "link-1".to_string(), hosted_link_url: "https://secure.plaid.com/hl/link-1".to_string() } },
        > Action::ToStore { action: ActionStore::SaveLink { call_id: 3, .. } },

        // The URL is handed over only once the row is down. Holding it until
        // the write lands is what makes "before" mean it.
        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: LinkOpened { url: "https://secure.plaid.com/hl/link-1" } }"#,
    );
}

/// B4b. A token that signs nobody in gets nowhere near Plaid.
///
/// Decided here rather than at the edge, which is what makes it a test at all:
/// `authenticate` used to 401 before any event existed. What the claim is
/// about is the action that does *not* come out — no Link session is opened
/// for a caller the store does not recognise.
#[test]
fn a_token_that_names_nobody_opens_no_session() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkRequested { token_sha256: hash_token("stale") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: None } },
        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// B4d. Plaid refused, was unreachable, or answered a `link_token` with no
/// hosted URL. All three arrive as one abandoned session with no token on it,
/// and the route is answered rather than left — there is no page to send
/// anybody to, and `/link/start` renders anything that is not a URL as a 502.
#[test]
fn a_session_plaid_would_not_open_is_not_a_page() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::OpenLink { call_id: 2, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::LinkAbandoned { call_id: 2, link_token: String::new() } },
        > @r#"ToStore { action: CollectLink { call_id: 3, link_token: "" } }"#,

        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: LinkAbandoned }"#,
    );
}
