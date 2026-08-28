//! The server's state machine, tested the way `docs/STYLE.md` says: events in
//! with `<`, actions out with `>`.
//!
//! What these pin is the server's side of a sync — that `/transactions/sync`
//! is a diff and not a snapshot, so a later call answering with nothing new
//! must not empty the ledger and a call re-sending an entry must not double
//! it — along with the two things that cost real money to get wrong: Plaid's
//! per-endpoint rate limit, and not retrying an Item whose login has expired.

use api_common::Timestamp;
use api_eval::eval;
use api_ledger::Connection;
use api_ledger::ConnectionStatus;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Tx;

use server_main::*;

const T0: Timestamp = Timestamp::from_secs(1_787_097_600);

fn state() -> StateServer {
    let mut state = StateServer::new();
    state.set_now(T0);
    state
}

fn entry(id: &str) -> Entry {
    Entry::from_tx(
        id.to_string(),
        Tx::Transfer {
            ts: Timestamp::from_secs(1_787_000_000),
            pos: Position {
                asset: "USD".into(),
                qty: -1.0,
            },
        },
    )
}

fn loaded(added: Vec<Entry>) -> Event {
    Event::FromSource {
        event: EventSource::PlaidLoaded {
            added,
            modified: vec![],
            removed: vec![],
            connections: vec![],
            streams: vec![],
            cursor: "cursor-1".into(),
        },
    }
}

fn failed(code: &str) -> Event {
    Event::FromSource {
        event: EventSource::PlaidFailed { code: code.into() },
    }
}

/// A server that came up with a store.
fn started() -> Event {
    started_with(true)
}

fn started_with(store: bool) -> Event {
    Event::FromProcess {
        event: EventProcess::Started { store },
    }
}

fn connection(id: &str, status: ConnectionStatus) -> Connection {
    Connection {
        id: id.to_string(),
        name: "First Platypus Bank".to_string(),
        initials: "FP".to_string(),
        status,
        accounts: vec![],
    }
}

fn read_back(entries: Vec<Entry>, cursor: Option<&str>, synced_at: Option<Timestamp>) -> Event {
    Event::FromStore {
        event: EventStore::Loaded {
            entries,
            connections: vec![connection(
                "item-1",
                ConnectionStatus::Ok {
                    synced_at: synced_at.unwrap_or(T0),
                },
            )],
            streams: vec![],
            cursor: cursor.map(str::to_string),
            synced_at,
        },
    }
}

fn fetched(item: &str, cursor: &str) -> Event {
    Event::FromSource {
        event: EventSource::PlaidLoaded {
            added: vec![entry("a")],
            modified: vec![],
            removed: vec![],
            connections: vec![connection(item, ConnectionStatus::Ok { synced_at: T0 })],
            streams: vec![],
            cursor: cursor.into(),
        },
    }
}

fn requested(force: bool) -> Event {
    Event::FromClient {
        event: EventClient::SnapshotRequested { force },
    }
}

#[test]
fn the_first_request_goes_to_plaid() {
    let mut state = state();

    eval!(&mut state,
        < requested(false),
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: None } },
    );
}

#[test]
fn a_fresh_snapshot_is_served_without_asking_plaid_again() {
    let mut state = state();

    eval!(&mut state,
        < loaded(vec![]),
        > Action::ToClient { action: ActionClient::Serve },

        < requested(false),
        > Action::ToClient { action: ActionClient::Serve },
    );
}

#[test]
fn a_stale_snapshot_is_refetched() {
    let mut state = state();

    eval!(&mut state,
        < loaded(vec![]),
        > Action::ToClient { action: ActionClient::Serve },
    );

    // The point of the test: enough time has passed that the held snapshot
    // is stale, so the next request goes back to Plaid.
    state.set_now(T0 + std::time::Duration::from_secs(FRESH_FOR_SECS));

    eval!(&mut state,
        < requested(false),
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: Some(_) } },
    );
}

#[test]
fn a_forced_request_refetches_however_fresh() {
    let mut state = state();

    eval!(&mut state,
        < loaded(vec![]),
        > Action::ToClient { action: ActionClient::Serve },

        < requested(true),
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: Some(_) } },
    );
}

/// Retrying a login-required Item cannot help, and Plaid counts the call.
#[test]
fn a_login_required_item_is_not_retried_even_when_forced() {
    let mut state = state();

    eval!(&mut state,
        < failed("ITEM_LOGIN_REQUIRED"),
        > Action::ToProcess { action: ActionProcess::ReportLoginRequired },
        > Action::ToClient { action: ActionClient::Serve },

        < requested(true),
        > Action::ToClient { action: ActionClient::Serve },
    );

    assert!(state.login_required());
}

#[test]
fn a_successful_fetch_clears_login_required() {
    let mut state = state();

    eval!(&mut state,
        < failed("ITEM_LOGIN_REQUIRED"),
        > Action::ToProcess { action: ActionProcess::ReportLoginRequired },
        > Action::ToClient { action: ActionClient::Serve },

        < loaded(vec![]),
        > Action::ToClient { action: ActionClient::Serve },
    );

    assert!(!state.login_required());
}

/// An outage must not turn into a hot loop against Plaid.
#[test]
fn a_transient_failure_still_counts_as_an_attempt() {
    let mut state = state();

    eval!(&mut state,
        < failed("INTERNAL_SERVER_ERROR"),
        > Action::ToClient { action: ActionClient::Serve },

        < requested(false),
        > Action::ToClient { action: ActionClient::Serve },
    );

    assert!(!state.login_required(), "only a login error is sticky");
}

/// The trap that emptied the app on its second fetch: `/transactions/sync`
/// is a diff, so a later call legitimately answers with nothing new.
#[test]
fn a_second_sync_with_nothing_new_keeps_the_ledger() {
    let mut state = state();

    eval!(&mut state,
        < loaded(vec![entry("a"), entry("b")]),
        > Action::ToClient { action: ActionClient::Serve },

        < loaded(vec![]),
        > Action::ToClient { action: ActionClient::Serve },
    );

    assert_eq!(state.entries().len(), 2, "the ledger must not empty itself");
}

/// The mirror of the trap above. A sync that answers with an entry the
/// ledger already holds must land on the one it holds, not beside it —
/// otherwise the same transaction is on screen twice.
#[test]
fn a_resent_entry_lands_on_the_one_already_held() {
    let mut state = state();

    eval!(&mut state,
        < loaded(vec![entry("a"), entry("b")]),
        > Action::ToClient { action: ActionClient::Serve },

        < loaded(vec![entry("a")]),
        > Action::ToClient { action: ActionClient::Serve },
    );

    assert_eq!(state.entries().len(), 2, "a transaction must appear once");
}

#[test]
fn a_later_sync_adds_to_what_is_held() {
    let mut state = state();

    eval!(&mut state,
        < loaded(vec![entry("a")]),
        > Action::ToClient { action: ActionClient::Serve },

        < Event::FromSource {
            event: EventSource::PlaidLoaded {
                added: vec![entry("b")],
                modified: vec![],
                removed: vec!["a".to_string()],
                connections: vec![],
                streams: vec![],
                cursor: "c2".into(),
            },
        },
        > Action::ToClient { action: ActionClient::Serve },
    );

    let ids: Vec<&str> = state.entries().iter().map(|e| e.id.as_str()).collect();

    assert_eq!(ids, ["b"], "added joins, removed leaves");
}

/// Plaid meters per endpoint; a burst has to be refused rather than sent
/// and rejected, because a rejected call still counts against the budget.
#[test]
fn a_burst_of_forced_refreshes_is_rate_limited() {
    let mut state = state();
    let mut actions = Vec::new();

    // BalanceGet allows 10/min, so the 11th forced refresh has no budget.
    let mut fetches = 0;

    for _ in 0..12 {
        actions.clear();
        state.handle_events(T0, &[requested(true)], &mut actions);

        if actions.iter().any(|a| matches!(a, Action::ToSource { .. })) {
            fetches += 1;
        }
    }

    assert_eq!(fetches, 10, "the endpoint's per-minute budget");
}

#[test]
fn shutting_down_stops_new_work() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromTerminal { event: EventTerminal::SigInt },
        > Action::ToProcess { action: ActionProcess::Shutdown },

        < requested(true),
        > Action::ToClient { action: ActionClient::Serve },
    );

    assert!(state.shutting_down());
}

/// A second ^C while the first is draining should not queue another.
#[test]
fn shutdown_is_idempotent() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromTerminal { event: EventTerminal::SigInt },
        < Event::FromTerminal { event: EventTerminal::SigTerm },
        > Action::ToProcess { action: ActionProcess::Shutdown },
    );
}

/// The store is read before anything is fetched. Whatever is on disk is what
/// the last run ended with, and starting from it is the difference between a
/// restart costing nothing and one re-downloading a history.
#[test]
fn starting_reads_the_store() {
    let mut state = state();

    eval!(&mut state,
        < started(),
        > Action::ToStore { action: ActionStore::LoadEverything },
    );
}

/// No store, nothing asked of it. This used to be a branch in the runtime,
/// which meant it could not be written down here at all.
#[test]
fn starting_without_a_store_asks_for_nothing() {
    let mut state = state();

    eval!(&mut state,
        < started_with(false),
    );
}

/// And nothing is written to a store that is not there, either.
#[test]
fn a_sync_with_no_store_is_served_and_not_written() {
    let mut state = state();

    eval!(&mut state,
        < started_with(false),

        < fetched("item-1", "cursor-2"),
        > Action::ToClient { action: ActionClient::Serve },
    );
}

/// Reading it back asks for nothing. Nobody is waiting at boot, and the data
/// is already what the next request would be served.
#[test]
fn a_read_store_asks_for_nothing() {
    let mut state = state();

    eval!(&mut state,
        < read_back(vec![entry("a"), entry("b")], Some("cursor-1"), Some(T0)),
    );

    assert_eq!(state.entries().len(), 2);
    assert_eq!(state.connections().len(), 1);
}

/// The whole point of persisting: a restart inside the freshness window serves
/// what it read instead of going straight back to Plaid for data it has.
#[test]
fn a_restart_with_fresh_data_does_not_go_back_to_plaid() {
    let mut state = state();

    eval!(&mut state,
        < read_back(vec![entry("a")], Some("cursor-1"), Some(T0)),

        < requested(false),
        > Action::ToClient { action: ActionClient::Serve },
    );
}

/// And one with stale data does, resuming from the cursor it read rather than
/// from the beginning.
#[test]
fn a_restart_with_stale_data_resumes_from_the_stored_cursor() {
    let mut state = state();

    let long_ago = Timestamp::from_secs(T0.as_secs() - 3_600);

    eval!(&mut state,
        < read_back(vec![entry("a")], Some("cursor-1"), Some(long_ago)),

        < requested(false),
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: Some(_) } },
    );
}

/// A sync is served first and written second: the request in hand should not
/// wait on a database round trip for data the server is already holding.
#[test]
fn a_sync_is_served_before_it_is_written() {
    let mut state = state();

    eval!(&mut state,
        < started(),
        > Action::ToStore { action: ActionStore::LoadEverything },

        < fetched("item-1", "cursor-2"),
        > Action::ToClient { action: ActionClient::Serve },
        > Action::ToStore { action: ActionStore::SaveFetched { .. } },
    );
}

/// Nothing to write it against. A fetch that produced no connection is a
/// fixture run or a failure, and inventing an Item id for it would put a row
/// in the database that no bank corresponds to.
#[test]
fn a_sync_with_no_connection_writes_nothing() {
    let mut state = state();

    eval!(&mut state,
        < loaded(vec![entry("a")]),
        > Action::ToClient { action: ActionClient::Serve },
    );
}

/// An expired login has to outlive the process, or the server comes back up
/// and asks Plaid the same question it was just refused.
#[test]
fn an_expired_login_is_remembered() {
    let mut state = state();

    eval!(&mut state,
        < started(),
        > Action::ToStore { action: ActionStore::LoadEverything },

        < read_back(vec![], Some("cursor-1"), Some(T0)),

        < failed("ITEM_LOGIN_REQUIRED"),
        > Action::ToProcess { action: ActionProcess::ReportLoginRequired },
        > Action::ToStore { action: ActionStore::SaveStatus { .. } },
        > Action::ToClient { action: ActionClient::Serve },
    );
}

/// And one read back from the store is believed, so a restart does not undo
/// the decision not to retry.
#[test]
fn an_expired_login_read_back_still_stops_the_retry() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromStore {
            event: EventStore::Loaded {
                entries: vec![],
                connections: vec![connection("item-1", ConnectionStatus::LoginRequired)],
                streams: vec![],
                cursor: Some("cursor-1".to_string()),
                synced_at: None,
            },
        },

        // Not even a forced refresh: no amount of asking fixes a login.
        < requested(true),
        > Action::ToClient { action: ActionClient::Serve },
    );

    assert!(state.login_required());
}

/// A store that cannot be read is survivable. An empty ledger is what a first
/// run has, and the next request fills it.
#[test]
fn a_store_that_cannot_be_read_still_serves() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromStore {
            event: EventStore::LoadFailed { message: "connection refused".to_string() },
        },

        < requested(false),
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: None } },
    );
}
