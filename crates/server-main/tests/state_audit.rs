//! The audit log's rule, which guards every flow at once: an answer that
//! serves customer data writes down who it was served to, and an answer that
//! serves none writes nothing.
//!
//! The flows already show the row going out beside their answers; what this
//! file pins is the row itself — whose data, what of it, from where, when —
//! and the two ways the rule must stay quiet: a refusal, and a token that
//! names nobody.

mod fixtures;

use api_eval::eval;
use fixtures::state_subscribed;

use server_main::*;

/// The row, whole. The user it names is the one the session resolved to, the
/// address is the caller's bucket, and the clock is the machine's.
#[test]
fn a_served_snapshot_is_written_down() {
    let mut state = state_subscribed();

    eval!(&mut state,
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

        > Action::ToStore {
            call_id: 3,
            action: ActionStore::LoadSnapshot { .. },
        },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::SnapshotLoaded {
                snapshot: server_wire::WireSnapshot::default(),
            },
        },

        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },

        > @r#"ToStore { call_id: 4, action: RecordAccess { user_id: 00000000-0000-0000-0000-000000000000, scope: Snapshot, client: "203.0.113.7", at: Timestamp { t: 1787097600 } } }"#,
    );
}

/// A refusal serves nothing, so it records nothing — here the paywall, which
/// answers a signed-in user without reading a row of theirs.
#[test]
fn a_refusal_writes_nothing() {
    let mut state = fixtures::state();

    eval!(&mut state,
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
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToRouter { action: ActionRouter::PaymentRequired { .. }, .. },
    );
}

/// A token naming nobody is refused before there is anybody to write about.
#[test]
fn a_stranger_writes_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SnapshotRequested {
                token_sha256: hash_token("nobody"),
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
