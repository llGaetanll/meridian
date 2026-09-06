//! Flow D of `docs/API_FLOW.md` — the first sync, and watching it — tested the
//! way `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! D starts in the same turn the Item is written down, before `/link/finish`
//! has even answered, and the phone is written to from the sync screen until
//! the server says the three Plaid calls are through. Which is why the sync
//! here is started by the store reporting the bank rather than by a request:
//! nobody asks for D to begin.
//!
//! The three calls themselves are the runtime's — `/accounts/balance/get`,
//! `/transactions/sync`, `/transactions/recurring/get` — and they reach the
//! machine as one `PlaidProgressed` per stage and one `PlaidLoaded` at the
//! end. So D2, D6 and D8 are read here as the events they arrive on rather
//! than as calls.
//!
//! D4b and D4c — no session, opened faster than the limit allows — are
//! `flow_e_the_ledger.rs` and `state_limits.rs`, which pin them once for every
//! authenticated route. The other half of D4c, ten already open, is here.

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

/// Whose bank every one-user test here is about.
const AL: uuid::Uuid = uuid::Uuid::nil();

/// D, end to end: the bank is written down, the sync starts, the phone opens
/// its stream, and every stage after that is written to it unasked.
///
/// The gap this closes used to be real and it cost a screen: the phone asked
/// how the sync was going in the instant after `/link/finish` answered, was
/// told nothing was running — which reads identically to "it finished" — and
/// decided the sync was over before Plaid had been called.
#[test]
fn a_first_sync_is_pushed_from_the_fetch_to_the_totals() {
    let mut state = state();

    eval!(&mut state,
        // C's last turn: the bank is written down, and the write says whose
        // it is. That is what D1 starts from, on a call nobody is parked on.
        < Event::FromStore { call_id: 99, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: AL, cursor: None, synced_at: None, login_required: false }) } },
        > @r#"ToSource { action: FetchPlaid { call_id: 1, item_id: "item-9", cursor: None } }"#,

        // 3 and 4a. The stream opens on the news that a bank landed. Nothing
        // has been reported yet, so the counts are zero — but `finished` is
        // what the screen reads, and it says no.
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncWatchRequested { conn_id: 1, token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::SessionResolved { user_id: Some(AL) } },
        > @r#"ToRouter { request_id: 1, action: Watching { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: false } } }"#,

        // 2. The balances answered, which is the first place the bank's name
        // exists — it is in neither the Link session nor the exchange. Nobody
        // asked for this and it goes out anyway, which is the whole point.
        < Event::FromSource { event: EventSource::PlaidProgressed { item_id: "item-9".to_string(), stage: server_wire::SyncStage::Transactions, institution: Some("First Platypus Bank".to_string()), accounts: 2, transactions: 0, streams: 0 } },
        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "transactions", institution: Some("First Platypus Bank"), accounts: 2, transactions: 0, streams: 0, finished: false } } }"#,

        // 6a and 8a. The whole fetch answers once, and the diff goes to the
        // store — the machine holds no ledger to apply it to. The totals reach
        // the screen that says "You're set." on the same turn.
        < Event::FromSource { event: EventSource::PlaidLoaded { call_id: 99, item_id: "item-9".to_string(), added: vec![Entry::from_tx("a".to_string(), Tx::Transfer { ts: Timestamp::from_secs(1_787_000_000), pos: Position { asset: "USD".into(), qty: -1.0 } })], modified: vec![], removed: vec![], connection: Some(Connection { id: "item-9".to_string(), name: "First Platypus Bank".to_string(), initials: "FP".to_string(), status: api_ledger::ConnectionStatus::Ok { synced_at: T0 }, accounts: vec![] }), streams: vec![], cursor: "cursor-2".into() } },
        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "streams", institution: Some("First Platypus Bank"), accounts: 0, transactions: 1, streams: 0, finished: true } } }"#,
        > Action::ToStore { action: ActionStore::SaveFetched { .. } },
    );
}

/// D4a for somebody who has never linked a bank. Nothing running is an answer
/// and not a missing resource: they are told so and left connected, because a
/// bank landing a moment later is exactly what they are waiting for.
#[test]
fn a_user_with_nothing_syncing_is_told_so_and_kept() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncWatchRequested { conn_id: 1, token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(AL) } },
        > @r#"ToRouter { request_id: 1, action: Watching { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,

        // Still connected, so the sync that starts afterwards reaches them
        // without their asking again.
        < Event::FromStore { call_id: 98, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: AL, cursor: None, synced_at: None, login_required: false }) } },
        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: false } } }"#,
        > Action::ToSource { action: ActionSource::FetchPlaid { .. } },
    );
}

/// A sync is pushed to the person whose bank it is and to nobody else. Two
/// accounts share a process and share no rows, and the owner rides on the
/// fetch rather than being looked up when there is somebody to tell.
#[test]
fn a_sync_reaches_only_the_user_whose_bank_it_is() {
    let mut state = state();

    let mia = uuid::Uuid::from_u128(2);

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncWatchRequested { conn_id: 1, token_sha256: hash_token("session-mia") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(mia) } },
        > @r#"ToRouter { request_id: 1, action: Watching { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,

        // Al's bank, Al's sync. Mia is watching and hears none of it.
        < Event::FromStore { call_id: 99, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: AL, cursor: None, synced_at: None, login_required: false }) } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. } },

        < Event::FromSource { event: EventSource::PlaidProgressed { item_id: "item-9".to_string(), stage: server_wire::SyncStage::Transactions, institution: Some("First Platypus Bank".to_string()), accounts: 8, transactions: 19, streams: 0 } },
    );
}

/// A fetch that died is no longer running, so what was watching it is told the
/// sync is over. Dropped rather than marked done — a screen holding a stream
/// on a fetch that failed would otherwise sit on its last stage for good.
#[test]
fn a_failed_sync_stops_being_something_to_watch() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromStore { call_id: 99, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: AL, cursor: None, synced_at: None, login_required: false }) } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncWatchRequested { conn_id: 1, token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::SessionResolved { user_id: Some(AL) } },
        > @r#"ToRouter { request_id: 1, action: Watching { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: false } } }"#,

        < Event::FromSource { event: EventSource::PlaidFailed { call_id: 1, item_id: "item-9".to_string(), code: "REQUEST_FAILED".to_string() } },
        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,
    );
}

/// A stream that has gone is not written to again. Which is what frees the
/// slot it was holding, so a phone that backgrounds all morning is not
/// eventually refused for the connections it already dropped.
#[test]
fn a_closed_stream_stops_being_written_to() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncWatchRequested { conn_id: 1, token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(AL) } },
        > Action::ToRouter { action: ActionRouter::Watching { .. }, .. },

        < Event::FromSocket { event: EventSocket::Closed { conn_id: 1 } },

        < Event::FromStore { call_id: 99, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: AL, cursor: None, synced_at: None, login_required: false }) } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. } },
    );
}

/// One caller may hold ten streams at once and not an eleventh. Refused
/// rather than closing the oldest: a client that cannot let go is a client
/// whose oldest connection is the one still working.
#[test]
fn an_eleventh_stream_is_refused() {
    let mut state = state();

    // The ten it is allowed, opened the ordinary way. Not the claim, so they
    // are reached rather than written out.
    for conn_id in 1..=WATCHERS_AT_MOST as u64 {
        // One call each, for the session read every authenticated route makes.
        let call_id = conn_id;

        state.handle_events(
            &[
                Event::FromRouter {
                    request_id: conn_id,
                    client: Client {
                        ip: ClientIp::new("203.0.113.7"),
                        user: None,
                    },
                    event: EventRouter::SyncWatchRequested {
                        conn_id,
                        token_sha256: hash_token("session-1"),
                    },
                },
                Event::FromStore {
                    call_id,
                    event: EventStore::SessionResolved { user_id: Some(AL) },
                },
            ],
            &mut Vec::new(),
        );
    }

    eval!(&mut state,
        < Event::FromRouter { request_id: 11, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncWatchRequested { conn_id: 11, token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 11, .. } },

        < Event::FromStore { call_id: 11, event: EventStore::SessionResolved { user_id: Some(AL) } },
        > @r#"ToRouter { request_id: 11, action: TooMany }"#,

        // Letting one go frees the slot, which is what stops a phone that has
        // backgrounded all morning being locked out by connections it dropped.
        < Event::FromSocket { event: EventSocket::Closed { conn_id: 1 } },

        < Event::FromRouter { request_id: 12, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SyncWatchRequested { conn_id: 12, token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 12, .. } },

        < Event::FromStore { call_id: 12, event: EventStore::SessionResolved { user_id: Some(AL) } },
        > Action::ToRouter { action: ActionRouter::Watching { conn_id: 12, .. }, .. },
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
