//! Flow D of `docs/API_FLOW.md` — the first sync, and watching it — tested the
//! way `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! D starts in the same turn the Item is written down, before `/link/finish`
//! has even answered, and the phone watches it from the sync screen until the
//! server says the three Plaid calls are through. Which is why nothing here
//! sends a request to start it: the fetch is already out.
//!
//! The three calls themselves are the runtime's — `/accounts/balance/get`,
//! `/transactions/sync`, `/transactions/recurring/get` — and they reach the
//! machine as one `PlaidProgressed` per stage and one `PlaidLoaded` at the
//! end. So D2, D6 and D8 are read here as the events they arrive on rather
//! than as calls.
//!
//! D4c and D4d — no session, too many polls — are `flow_e_the_ledger.rs` and
//! `state_limits.rs`, which pin them once for every authenticated route.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Tx;
use fixtures::T0;
use fixtures::state;

use server_main::*;

/// D. A sync watched from the moment it was asked for to the tick that says it
/// is over — the doc's 1 through 10, with the phone's polls at 3, 9 and one in
/// between.
///
/// The gap this closes used to be real and it cost a screen: the phone asked
/// how the sync was going in the instant after `/link/finish` answered, was
/// told nothing was running — which reads identically to "it finished" — and
/// decided the sync was over before Plaid had been called.
#[test]
fn a_first_sync_is_watched_from_the_fetch_to_the_totals() {
    let mut state = state();

    eval!(&mut state,
        // D1 has gone out. It was asked for on C's last turn, on a call
        // nobody is parked on — 99 here, standing for an id this test did not
        // watch being handed out — and this is the first stage landing.
        < Event::FromSource { event: EventSource::PlaidProgressed { item_id: "item-9".to_string(), stage: server_wire::SyncStage::Accounts, institution: None, accounts: 0, transactions: 0, streams: 0 } },

        // 3 and 4a. Nothing has been reported yet, so the counts are zero —
        // but `finished` is what the screen reads, and it says no.
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncStatusRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-9".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > @r#"ToRouter { request_id: 1, action: SyncStatus { status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: false } } }"#,

        // 2. The balances answered, which is the first place the bank's name
        // exists — it is in neither the Link session nor the exchange.
        < Event::FromSource { event: EventSource::PlaidProgressed { item_id: "item-9".to_string(), stage: server_wire::SyncStage::Transactions, institution: Some("First Platypus Bank".to_string()), accounts: 2, transactions: 0, streams: 0 } },

        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncStatusRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 3, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 3, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 4, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 4, items: vec![ItemFacts { item_id: "item-9".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > @r#"ToRouter { request_id: 2, action: SyncStatus { status: WireSyncStatus { stage: "transactions", institution: Some("First Platypus Bank"), accounts: 2, transactions: 0, streams: 0, finished: false } } }"#,

        // 6a and 8a. The whole fetch answers once, and the diff goes to the
        // store — the machine holds no ledger to apply it to. Nobody is
        // waiting on it, so the write gets a fresh id rather than 99's.
        < Event::FromSource { event: EventSource::PlaidLoaded { call_id: 99, item_id: "item-9".to_string(), added: vec![Entry::from_tx("a".to_string(), Tx::Transfer { ts: Timestamp::from_secs(1_787_000_000), pos: Position { asset: "USD".into(), qty: -1.0 } })], modified: vec![], removed: vec![], connection: Some(Connection { id: "item-9".to_string(), name: "First Platypus Bank".to_string(), initials: "FP".to_string(), status: api_ledger::ConnectionStatus::Ok { synced_at: T0 }, accounts: vec![] }), streams: vec![], cursor: "cursor-2".into() } },
        > Action::ToStore { action: ActionStore::SaveFetched { call_id: 5, .. } },

        // 9 and 10. The last poll: nothing is running, and the totals are on
        // this reply. The screen that says "You're set." reads them from here.
        < Event::FromRouter { request_id: 3, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncStatusRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 6, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 6, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 7, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 7, items: vec![ItemFacts { item_id: "item-9".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > @r#"ToRouter { request_id: 3, action: SyncStatus { status: WireSyncStatus { stage: "streams", institution: Some("First Platypus Bank"), accounts: 0, transactions: 1, streams: 0, finished: true } } }"#,
    );
}

/// D4b for somebody who has never linked a bank. Nothing running is an answer,
/// not a missing resource: `finished` and zeroes rather than a 404.
#[test]
fn a_user_with_nothing_syncing_is_told_so() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncStatusRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![] } },
        > @r#"ToRouter { request_id: 1, action: SyncStatus { status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,
    );
}

/// Progress is read back against the *asking* user's own banks. The machine
/// holds it by item and has no idea whose item is whose, so a sync running for
/// somebody else is not this caller's to watch.
#[test]
fn a_sync_is_only_watched_by_the_user_whose_bank_it_is() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::PlaidProgressed { item_id: "item-9".to_string(), stage: server_wire::SyncStage::Transactions, institution: Some("First Platypus Bank".to_string()), accounts: 8, transactions: 19, streams: 0 } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncStatusRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-4".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > @r#"ToRouter { request_id: 1, action: SyncStatus { status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,
    );
}

/// A fetch that died is no longer running, so what was watching it is told the
/// sync is over. Dropped rather than marked done — a screen polling a fetch
/// that failed would otherwise sit on its last stage for good.
#[test]
fn a_failed_sync_stops_being_something_to_watch() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::PlaidProgressed { item_id: "item-9".to_string(), stage: server_wire::SyncStage::Transactions, institution: Some("First Platypus Bank".to_string()), accounts: 8, transactions: 19, streams: 0 } },

        < Event::FromSource { event: EventSource::PlaidFailed { call_id: 1, item_id: "item-9".to_string(), code: "REQUEST_FAILED".to_string() } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncStatusRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-9".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > @r#"ToRouter { request_id: 1, action: SyncStatus { status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,
    );
}

/// An expired login has to outlive the process, or the server comes back up
/// and asks Plaid the same question it was just refused. It is also what the
/// Reconnect button reads.
#[test]
fn an_expired_login_is_reported_and_written_down() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::PlaidFailed { call_id: 1, item_id: "item-1".to_string(), code: "ITEM_LOGIN_REQUIRED".to_string() } },
        > Action::ToProcess { action: ActionProcess::ReportLoginRequired },
        > Action::ToStore { action: ActionStore::SaveStatus { .. } },
    );
}

/// Any other failure is just a failure: nothing to write down, and the next
/// request will try again.
#[test]
fn an_ordinary_failure_writes_nothing() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::PlaidFailed { call_id: 1, item_id: "item-1".to_string(), code: "INTERNAL_SERVER_ERROR".to_string() } },
    );
}
