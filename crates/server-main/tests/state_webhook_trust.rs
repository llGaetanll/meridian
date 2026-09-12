//! Whether a POST to `/plaid/webhook` is really Plaid's — the checks behind
//! C2's steps 3, 4 and 7b in `docs/API_FLOW.md`, tested the way
//! `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! This is the one route with no session behind it, and the one route with no
//! rate limit, so the signature is the whole of what protects it. Every check
//! is the machine's — which algorithm is acceptable, whether a key is worth
//! fetching, how long a `kid` nobody has heard of stays refused, and whether
//! the signature is inside its replay window — because every one of them is a
//! rule, none of them needs a socket, and all of them are now something an
//! `eval!` test can reach.
//!
//! What a *believed* webhook is worth doing about is `flow_c2_…` and
//! `flow_f_…`. Nothing here gets that far.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use fixtures::T0;
use fixtures::signed;
use fixtures::state;
use fixtures::webhook_jwk;

use server_main::*;

/// A signature that does not check out is refused, and nothing is acted on.
///
/// The body says a Link session finished; the token was signed over different
/// bytes. What the claim is about is the actions that do *not* come out.
#[test]
fn a_webhook_signed_over_other_bytes_is_refused() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::WebhookReceived {
                jwt: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs(),
                )
                .0,
                body: b"{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\"}"
                    .to_vec(),
            },
        },

        > Action::ToSourcePlaid {
            call_id: 1,
            action: ActionSourcePlaid::FetchWebhookKey { .. },
        },
        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::WebhookKeyLoaded {
                kid: "kid-1".to_string(),
                key: Some(webhook_jwk()),
            },
        },

        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// A signature older than the replay window is refused however good it is.
///
/// The window is arithmetic on the tick, so this test is a clock and not a
/// stopwatch — which is the whole reason it moved out of the runtime.
#[test]
fn a_webhook_older_than_the_replay_window_is_refused() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::WebhookReceived {
                jwt: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs() - 600,
                )
                .0,
                body: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs() - 600,
                )
                .1,
            },
        },

        > Action::ToSourcePlaid {
            call_id: 1,
            action: ActionSourcePlaid::FetchWebhookKey { .. },
        },
        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::WebhookKeyLoaded {
                kid: "kid-1".to_string(),
                key: Some(webhook_jwk()),
            },
        },

        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// A `kid` Plaid does not have costs one call, and the next webhook naming it
/// costs none.
///
/// The whole point of remembering a miss: without it, an id anybody can invent
/// is an outbound request per POST, which is a denial of service pointed at us
/// by way of Plaid.
#[test]
fn a_key_plaid_does_not_have_is_asked_for_once() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::WebhookReceived {
                jwt: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs(),
                )
                .0,
                body: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs(),
                )
                .1,
            },
        },

        > Action::ToSourcePlaid {
            call_id: 1,
            action: ActionSourcePlaid::FetchWebhookKey { .. },
        },
        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::WebhookKeyLoaded {
                kid: "kid-1".to_string(),
                key: None,
            },
        },

        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,

        // The same id again. Refused without asking Plaid a second time.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::WebhookReceived {
                jwt: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs(),
                )
                .0,
                body: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs(),
                )
                .1,
            },
        },
        > @r#"ToRouter { request_id: 2, action: Unauthorized }"#,
    );
}

/// A miss expires, so a key we failed to fetch is asked for again.
///
/// The other half of the rule above, and the reason a miss is not remembered
/// forever: Plaid being down for a minute must not refuse a real key for the
/// life of the process.
#[test]
fn a_miss_is_asked_about_again_once_it_has_expired() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::WebhookReceived {
                jwt: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs(),
                )
                .0,
                body: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs(),
                )
                .1,
            },
        },

        > Action::ToSourcePlaid {
            call_id: 1,
            action: ActionSourcePlaid::FetchWebhookKey { .. },
        },
        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::WebhookKeyLoaded {
                kid: "kid-1".to_string(),
                key: None,
            },
        },

        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,

        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(
                    T0.as_secs() + WEBHOOK_KEY_MISS_GOOD_FOR_SECS,
                ),
            },
        },

        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::WebhookReceived {
                jwt: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs() + WEBHOOK_KEY_MISS_GOOD_FOR_SECS,
                )
                .0,
                body: signed(
                    "{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}",
                    T0.as_secs() + WEBHOOK_KEY_MISS_GOOD_FOR_SECS,
                )
                .1,
            },
        },

        > Action::ToSourcePlaid {
            call_id: 2,
            action: ActionSourcePlaid::FetchWebhookKey { .. },
        },
    );
}
