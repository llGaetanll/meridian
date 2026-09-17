//! Flow G of `docs/API_FLOW.md` — repairing a bank whose login has expired —
//! the way `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! A repair is Plaid's update mode: the same Hosted Link page, opened against
//! an Item we already hold. Nothing it produces is ever exchanged, because the
//! exchange is the call that spends a Production Item slot and the bank it
//! would buy is one we already have. What a successful repair changes is one
//! column — `error_code` back to `NULL` — which is what lets `should_fetch`
//! ask the bank again.

mod fixtures;

use api_eval::eval;
use fixtures::T0;
use fixtures::signed;
use fixtures::state_subscribed;
use fixtures::state_subscribed_by_webhook;
use fixtures::webhook_jwk;

use server_main::*;

/// G's happy path, first half: the repair is asked for, the caller's banks
/// say it is theirs and dead, and Plaid opens an update-mode session that is
/// written down as one.
#[test]
fn a_repair_opens_an_update_session_and_writes_down_whose_it_is() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::RepairRequested {
                token_sha256: hash_token("session-1"),
                origin: None,
                item_id: "item-9".to_string(),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        // The caller's banks, because whose Item it is has to be settled
        // before Plaid is asked for anything.
        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadItems { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::ItemsLoaded {
                items: vec![ItemFacts {
                    item_id: "item-9".to_string(),
                    user_id: uuid::Uuid::nil(),
                    cursor: Some("cursor-1".to_string()),
                    synced_at: Some(T0),
                    login_required: true,
                }],
            },
        },

        > @r#"ToSourcePlaid { call_id: 3, action: OpenRepair { user_id: 00000000-0000-0000-0000-000000000000, item_id: "item-9", opener: App } }"#,
        < Event::FromSourcePlaid {
            call_id: 3,
            event: EventSourcePlaid::LinkOpened {
                user_id: uuid::Uuid::nil(),
                link_token: "link-7".to_string(),
                hosted_link_url: "https://secure.plaid.com/hl/link-7"
                    .to_string(),
                repair_of: Some("item-9".to_string()),
            },
        },

        // The session row carries the Item, which is what will keep its
        // finish from being exchanged.
        > @r#"ToStore { call_id: 4, action: SaveLink { link_token: "link-7", user_id: 00000000-0000-0000-0000-000000000000, repair_of: Some("item-9") } }"#,
        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: LinkOpened { url: "https://secure.plaid.com/hl/link-7" } }"#,
    );
}

/// G's happy path, second half, on a server that polls: the sheet closes, the
/// session is a repair and has finished, and what happens is one write —
/// `error_code` cleared — and no exchange, whatever the poll carried.
///
/// The poll here answers with a token on the session, which is the sharpest
/// version of the claim: even then, nothing is exchanged.
#[test]
fn a_finished_repair_clears_the_error_and_exchanges_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::LinkClosed {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadPendingLinks { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::PendingLinksLoaded {
                links: vec![
                    PendingLinkFacts {
                        link_token: "link-7".to_string(),
                        repair_of: Some("item-9".to_string()),
                    },
                ],
            },
        },

        > Action::ToSourcePlaid {
            call_id: 3,
            action: ActionSourcePlaid::PollLinks { .. },
        },
        < Event::FromSourcePlaid {
            call_id: 3,
            event: EventSourcePlaid::LinksPolled {
                polls: vec![LinkPoll {
                    link_token: "link-7".to_string(),
                    finished: true,
                    public_tokens: vec!["public-7".to_string()],
                }],
            },
        },

        > @r#"ToStore { call_id: 4, action: SaveStatus { item_id: "item-9", error_code: None } }"#,

        > @r#"ToStore { call_id: 5, action: CollectLink { link_token: "link-7" } }"#,

        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },
        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Linked { count: 1 } }"#,
    );
}

/// A bank the caller does not hold is refused, before Plaid is asked for
/// anything. "Not yours" and "no such bank" are one answer on purpose:
/// telling them apart says which Items exist.
#[test]
fn a_repair_of_somebody_elses_bank_is_refused() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::RepairRequested {
                token_sha256: hash_token("session-1"),
                origin: None,
                item_id: "item-9".to_string(),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadItems { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::ItemsLoaded { items: vec![] },
        },

        > @r#"ToRouter { request_id: 1, action: Rejected }"#,
    );
}

/// A bank that answers has nothing to repair, and opening a session against
/// it would ask somebody to sign in for no reason.
#[test]
fn a_repair_of_a_bank_that_answers_is_refused() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::RepairRequested {
                token_sha256: hash_token("session-1"),
                origin: None,
                item_id: "item-9".to_string(),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadItems { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::ItemsLoaded {
                items: vec![ItemFacts {
                    item_id: "item-9".to_string(),
                    user_id: uuid::Uuid::nil(),
                    cursor: Some("cursor-1".to_string()),
                    synced_at: Some(T0),
                    login_required: false,
                }],
            },
        },

        > @r#"ToRouter { request_id: 1, action: Rejected }"#,
    );
}

/// The second half on a server Plaid calls: `SESSION_FINISHED` with
/// `status: SUCCESS`, on a session the store says is a repair. The error is
/// cleared and the outcome waits for the close, as `Linked`.
#[test]
fn a_repair_reported_by_webhook_clears_the_error_and_answers_linked() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::WebhookReceived {
                jwt: signed(
                    "{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"status\":\"SUCCESS\",\"link_token\":\"link-7\",\"public_tokens\":[]}",
                    T0.as_secs(),
                )
                .0,
                body: signed(
                    "{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"status\":\"SUCCESS\",\"link_token\":\"link-7\",\"public_tokens\":[]}",
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
                key: Some(webhook_jwk()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadLinkRepair { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::LinkRepairLoaded {
                repair_of: Some("item-9".to_string()),
            },
        },

        > @r#"ToStore { call_id: 3, action: SaveStatus { item_id: "item-9", error_code: None } }"#,
        < Event::FromStore { call_id: 3, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,

        // The phone catches up, and the recorded outcome is a repair that
        // worked.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::LinkClosed {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 4,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::LoadPendingLinks { .. },
        },
        < Event::FromStore {
            call_id: 5,
            event: EventStore::PendingLinksLoaded {
                links: vec![
                    PendingLinkFacts {
                        link_token: "link-7".to_string(),
                        repair_of: Some("item-9".to_string()),
                    },
                ],
            },
        },

        > Action::ToStore {
            call_id: 6,
            action: ActionStore::CollectLink { .. },
        },
        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 2, action: Linked { count: 1 } }"#,
    );
}

/// `status: EXITED` is somebody closing the sheet without signing in. Nothing
/// is cleared — the bank still does not answer — and the close hears the
/// ordinary abandonment.
#[test]
fn a_repair_they_backed_out_of_clears_nothing() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::WebhookReceived {
                jwt: signed(
                    "{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"status\":\"EXITED\",\"link_token\":\"link-7\",\"public_tokens\":[]}",
                    T0.as_secs(),
                )
                .0,
                body: signed(
                    "{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"status\":\"EXITED\",\"link_token\":\"link-7\",\"public_tokens\":[]}",
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
                key: Some(webhook_jwk()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadLinkRepair { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::LinkRepairLoaded {
                repair_of: Some("item-9".to_string()),
            },
        },

        > @r#"ToRouter { request_id: 1, action: Accepted }"#,

        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::LinkClosed {
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 4,
            action: ActionStore::LoadPendingLinks { .. },
        },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::PendingLinksLoaded {
                links: vec![
                    PendingLinkFacts {
                        link_token: "link-7".to_string(),
                        repair_of: Some("item-9".to_string()),
                    },
                ],
            },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::CollectLink { .. },
        },
        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },

        > @r#"ToRouter { request_id: 2, action: LinkAbandoned }"#,
    );
}
