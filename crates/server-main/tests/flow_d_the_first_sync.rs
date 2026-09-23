//! Flow D of `docs/API_FLOW.md` — the first sync, and watching it — tested the
//! way `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! D starts in the same turn the Item is written down, before `/link/finish`
//! has even answered, and the phone is written to from the sync screen until
//! the server says the three Plaid calls are through. Which is why the sync
//! here is started by the store reporting the bank rather than by a request:
//! nobody asks for D to begin.
//!
//! The three calls are the machine's own — `/accounts/get`,
//! `/transactions/sync`, `/transactions/recurring/get` — one action and one
//! answer each, in that order, with each answer deciding what is asked next.
//! So D1 through D8 are read here as the actions and events they are.
//!
//! D4b and D4c — no session, opened faster than the limit allows — are
//! `flow_e_the_ledger.rs` and `state_limits.rs`, which pin them once for every
//! authenticated route. The other half of D4c, ten already open, is here.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use api_ledger::Account;
use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Tx;
use fixtures::T0;
use fixtures::state_subscribed;
use fixtures::state_subscribed_many;

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
    let mut state = state_subscribed();

    eval!(&mut state,
        // C's last turn: the bank is written down, and the write says whose
        // it is. That is what D1 starts from, on a call nobody is parked on.
        < Event::FromStore {
            call_id: 99,
            event: EventStore::ItemSaved {
                item: Some(ItemFacts {
                    item_id: "item-9".to_string(),
                    user_id: AL,
                    cursor: None,
                    synced_at: None,
                    login_required: false,
                }),
            },
        },

        > @r#"ToSourcePlaid { call_id: 1, action: FetchBalances { item_id: "item-9", now: Timestamp { t: 1787097600 } } }"#,

        // 3 and 4a. The stream opens on the news that a bank landed. Nothing
        // has been reported yet, so the counts are zero — but `finished` is
        // what the screen reads, and it says no.
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SyncWatchRequested {
                conn_id: 1,
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::SessionResolved { user_id: Some(AL) },
        },

        > @r#"ToRouter { request_id: 1, action: Watching { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: false } } }"#,

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },

        // 2. The balances answered, which is the first place the bank's name
        // exists — it is in neither the Link session nor the exchange. Nobody
        // asked for this and it goes out anyway, which is the whole point.
        // 5 follows on the same turn: what to ask next is the machine's.
        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::BalancesLoaded {
                item_id: "item-9".to_string(),
                connection: Connection {
                    id: "item-9".to_string(),
                    name: "First Platypus Bank".to_string(),
                    initials: "FP".to_string(),
                    status: api_ledger::ConnectionStatus::Ok { synced_at: T0 },
                    accounts: vec![
                        Account {
                            id: "acct-1".to_string(),
                            name: "Checking".to_string(),
                            subtype: None,
                            mask: None,
                            balance: 0.0,
                            available: None,
                            limit: None,
                            apy: None,
                            group: api_ledger::AccountGroup::Depository,
                        },
                        Account {
                            id: "acct-2".to_string(),
                            name: "Savings".to_string(),
                            subtype: None,
                            mask: None,
                            balance: 0.0,
                            available: None,
                            limit: None,
                            apy: None,
                            group: api_ledger::AccountGroup::Depository,
                        },
                    ],
                },
            },
        },

        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "transactions", institution: Some("First Platypus Bank"), accounts: 2, transactions: 0, streams: 0, finished: false } } }"#,

        > Action::ToStore { action: ActionStore::SaveAccounts { .. }, .. },

        // 6a. One page, and it is the last one, so 7 goes out behind it. The
        // diff goes to the store — the machine holds no ledger to apply it to.
        > @r#"ToSourcePlaid { call_id: 5, action: SyncTransactions { item_id: "item-9", cursor: None } }"#,
        < Event::FromSourcePlaid {
            call_id: 5,
            event: EventSourcePlaid::TransactionsLoaded {
                item_id: "item-9".to_string(),
                added: vec![Entry::from_tx(
                    "a".to_string(),
                    Tx::Transfer {
                        ts: Timestamp::from_secs(1_787_000_000),
                        pos: Position { asset: "USD".into(), qty: -1.0 },
                    },
                )],
                modified: vec![],
                removed: vec![],
                cursor: "cursor-2".to_string(),
                has_more: false,
            },
        },

        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "streams", institution: Some("First Platypus Bank"), accounts: 2, transactions: 1, streams: 0, finished: false } } }"#,

        > Action::ToStore { action: ActionStore::SaveTransactions { .. }, .. },

        // 8a and 10. The last answer, and the totals reach the screen that
        // says "You're set." on the same turn.
        > @r#"ToSourcePlaid { call_id: 7, action: FetchRecurring { item_id: "item-9" } }"#,
        < Event::FromSourcePlaid {
            call_id: 7,
            event: EventSourcePlaid::RecurringLoaded {
                item_id: "item-9".to_string(),
                streams: vec![],
            },
        },

        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "streams", institution: Some("First Platypus Bank"), accounts: 2, transactions: 1, streams: 0, finished: true } } }"#,

        > Action::ToStore { action: ActionStore::SaveStreams { .. }, .. },
    );
}

/// D5 and D6a. A history longer than one page is more than one call, and the
/// one after the first carries the cursor the first came back with.
///
/// Following that chain is the machine's, which is what makes it countable: a
/// loop somewhere in the IO would be four calls behind what reads as one, and
/// the counts on the screen would be the last page rather than all of them.
#[test]
fn a_history_that_pages_is_asked_for_one_page_at_a_time() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromStore {
            call_id: 99,
            event: EventStore::ItemSaved {
                item: Some(ItemFacts {
                    item_id: "item-9".to_string(),
                    user_id: AL,
                    cursor: None,
                    synced_at: None,
                    login_required: false,
                }),
            },
        },

        > Action::ToSourcePlaid {
            call_id: 1,
            action: ActionSourcePlaid::FetchBalances { .. },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SyncWatchRequested {
                conn_id: 1,
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::SessionResolved { user_id: Some(AL) },
        },

        > Action::ToRouter { action: ActionRouter::Watching { .. }, .. },

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },

        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::BalancesLoaded {
                item_id: "item-9".to_string(),
                connection: Connection {
                    id: "item-9".to_string(),
                    name: "First Platypus Bank".to_string(),
                    initials: "FP".to_string(),
                    status: api_ledger::ConnectionStatus::Ok { synced_at: T0 },
                    accounts: vec![],
                },
            },
        },

        > Action::ToSocket { action: ActionSocket::Send { .. } },

        > Action::ToStore { action: ActionStore::SaveAccounts { .. }, .. },

        // More behind it, so the page is written down and the next one asked
        // for from where this one stopped.
        > @r#"ToSourcePlaid { call_id: 5, action: SyncTransactions { item_id: "item-9", cursor: None } }"#,
        < Event::FromSourcePlaid {
            call_id: 5,
            event: EventSourcePlaid::TransactionsLoaded {
                item_id: "item-9".to_string(),
                added: vec![Entry::from_tx(
                    "a".to_string(),
                    Tx::Transfer {
                        ts: Timestamp::from_secs(1_787_000_000),
                        pos: Position { asset: "USD".into(), qty: -1.0 },
                    },
                )],
                modified: vec![],
                removed: vec![],
                cursor: "cursor-2".to_string(),
                has_more: true,
            },
        },

        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "transactions", institution: Some("First Platypus Bank"), accounts: 0, transactions: 1, streams: 0, finished: false } } }"#,

        > Action::ToStore { action: ActionStore::SaveTransactions { .. }, .. },

        // The last one. What the screen is told is both pages and not the
        // second, which is the count nobody else is holding.
        > @r#"ToSourcePlaid { call_id: 7, action: SyncTransactions { item_id: "item-9", cursor: Some("cursor-2") } }"#,
        < Event::FromSourcePlaid {
            call_id: 7,
            event: EventSourcePlaid::TransactionsLoaded {
                item_id: "item-9".to_string(),
                added: vec![Entry::from_tx(
                    "b".to_string(),
                    Tx::Transfer {
                        ts: Timestamp::from_secs(1_787_000_000),
                        pos: Position { asset: "USD".into(), qty: -1.0 },
                    },
                )],
                modified: vec![],
                removed: vec![],
                cursor: "cursor-3".to_string(),
                has_more: false,
            },
        },

        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "streams", institution: Some("First Platypus Bank"), accounts: 0, transactions: 2, streams: 0, finished: false } } }"#,

        > Action::ToStore { action: ActionStore::SaveTransactions { .. }, .. },

        > @r#"ToSourcePlaid { call_id: 9, action: FetchRecurring { item_id: "item-9" } }"#,
    );
}

/// D8b. The recurring call is the one a sync can do without: it is logged and
/// carried on from, and the sync stands with what it got.
///
/// A fresh Item answers `PRODUCT_NOT_READY` here for the first seconds of its
/// life, because Plaid derives the streams from transactions that have only
/// just landed. Transactions are what the app is for, so a missing list of
/// charges costs one screen rather than all of them.
#[test]
fn a_failed_recurring_call_leaves_the_sync_finished() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromStore {
            call_id: 99,
            event: EventStore::ItemSaved {
                item: Some(ItemFacts {
                    item_id: "item-9".to_string(),
                    user_id: AL,
                    cursor: None,
                    synced_at: None,
                    login_required: false,
                }),
            },
        },

        > Action::ToSourcePlaid {
            call_id: 1,
            action: ActionSourcePlaid::FetchBalances { .. },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SyncWatchRequested {
                conn_id: 1,
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::SessionResolved { user_id: Some(AL) },
        },

        > Action::ToRouter { action: ActionRouter::Watching { .. }, .. },

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },

        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::BalancesLoaded {
                item_id: "item-9".to_string(),
                connection: Connection {
                    id: "item-9".to_string(),
                    name: "First Platypus Bank".to_string(),
                    initials: "FP".to_string(),
                    status: api_ledger::ConnectionStatus::Ok { synced_at: T0 },
                    accounts: vec![],
                },
            },
        },

        > Action::ToSocket { action: ActionSocket::Send { .. } },

        > Action::ToStore { action: ActionStore::SaveAccounts { .. }, .. },

        > Action::ToSourcePlaid {
            call_id: 5,
            action: ActionSourcePlaid::SyncTransactions { .. },
        },
        < Event::FromSourcePlaid {
            call_id: 5,
            event: EventSourcePlaid::TransactionsLoaded {
                item_id: "item-9".to_string(),
                added: vec![Entry::from_tx(
                    "a".to_string(),
                    Tx::Transfer {
                        ts: Timestamp::from_secs(1_787_000_000),
                        pos: Position { asset: "USD".into(), qty: -1.0 },
                    },
                )],
                modified: vec![],
                removed: vec![],
                cursor: "cursor-2".to_string(),
                has_more: false,
            },
        },

        > Action::ToSocket { action: ActionSocket::Send { .. } },

        > Action::ToStore { action: ActionStore::SaveTransactions { .. }, .. },

        // Finished, with the transactions it did get. Nothing is written down
        // about it: there is no list of charges to write and nothing to fix.
        > Action::ToSourcePlaid {
            call_id: 7,
            action: ActionSourcePlaid::FetchRecurring { .. },
        },
        < Event::FromSourcePlaid {
            call_id: 7,
            event: EventSourcePlaid::PlaidFailed {
                item_id: "item-9".to_string(),
                stage: server_wire::SyncStage::Streams,
                fault: src_plaid::PlaidFault::NotReady,
            },
        },

        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "streams", institution: Some("First Platypus Bank"), accounts: 0, transactions: 1, streams: 0, finished: true } } }"#,
    );
}

/// D6b. Sandbox generates transactions asynchronously, so a first sync's
/// `/transactions/sync` can answer `PRODUCT_NOT_READY`. The machine asks
/// again — at 5, 15, 30 and 60 seconds, on the tick — and the sync stays
/// something to watch the whole time, because "the bank is attached and its
/// history has not arrived yet" is not finished. When the last retry fails
/// too, the sync is marked failed the way any dead fetch is: dropped, and the
/// watcher told it is over.
#[test]
fn a_transactions_call_that_is_not_ready_is_asked_again() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromStore {
            call_id: 99,
            event: EventStore::ItemSaved {
                item: Some(ItemFacts {
                    item_id: "item-9".to_string(),
                    user_id: AL,
                    cursor: None,
                    synced_at: None,
                    login_required: false,
                }),
            },
        },

        > Action::ToSourcePlaid {
            call_id: 1,
            action: ActionSourcePlaid::FetchBalances { .. },
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SyncWatchRequested {
                conn_id: 1,
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::SessionResolved { user_id: Some(AL) },
        },

        > Action::ToRouter { action: ActionRouter::Watching { .. }, .. },

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },

        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::BalancesLoaded {
                item_id: "item-9".to_string(),
                connection: Connection {
                    id: "item-9".to_string(),
                    name: "First Platypus Bank".to_string(),
                    initials: "FP".to_string(),
                    status: api_ledger::ConnectionStatus::Ok { synced_at: T0 },
                    accounts: vec![],
                },
            },
        },

        > Action::ToSocket { action: ActionSocket::Send { .. } },

        > Action::ToStore { action: ActionStore::SaveAccounts { .. }, .. },

        > Action::ToSourcePlaid {
            call_id: 5,
            action: ActionSourcePlaid::SyncTransactions { .. },
        },

        // Not ready is not failure: the sync keeps running and the watcher
        // hears nothing, because nothing it is told about has changed.
        < Event::FromSourcePlaid {
            call_id: 5,
            event: EventSourcePlaid::PlaidFailed {
                item_id: "item-9".to_string(),
                stage: server_wire::SyncStage::Transactions,
                fault: src_plaid::PlaidFault::NotReady,
            },
        },

        // A tick before the five seconds are up asks nothing.
        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(T0.as_secs() + 4),
            },
        },

        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(T0.as_secs() + 5),
            },
        },

        > @r#"ToSourcePlaid { call_id: 6, action: SyncTransactions { item_id: "item-9", cursor: None } }"#,
        < Event::FromSourcePlaid {
            call_id: 6,
            event: EventSourcePlaid::PlaidFailed {
                item_id: "item-9".to_string(),
                stage: server_wire::SyncStage::Transactions,
                fault: src_plaid::PlaidFault::NotReady,
            },
        },

        // The wait grows: fifteen seconds this time, measured from the
        // failure rather than from the first ask.
        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(T0.as_secs() + 5 + 14),
            },
        },

        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(T0.as_secs() + 5 + 15),
            },
        },

        > @r#"ToSourcePlaid { call_id: 7, action: SyncTransactions { item_id: "item-9", cursor: None } }"#,
        < Event::FromSourcePlaid {
            call_id: 7,
            event: EventSourcePlaid::PlaidFailed {
                item_id: "item-9".to_string(),
                stage: server_wire::SyncStage::Transactions,
                fault: src_plaid::PlaidFault::NotReady,
            },
        },

        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(T0.as_secs() + 20 + 30),
            },
        },

        > @r#"ToSourcePlaid { call_id: 8, action: SyncTransactions { item_id: "item-9", cursor: None } }"#,
        < Event::FromSourcePlaid {
            call_id: 8,
            event: EventSourcePlaid::PlaidFailed {
                item_id: "item-9".to_string(),
                stage: server_wire::SyncStage::Transactions,
                fault: src_plaid::PlaidFault::NotReady,
            },
        },

        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(T0.as_secs() + 50 + 60),
            },
        },

        > @r#"ToSourcePlaid { call_id: 9, action: SyncTransactions { item_id: "item-9", cursor: None } }"#,

        // The fourth retry was the last. This failure is a failure, and the
        // watcher is told the sync is over the way any dead fetch says it.
        < Event::FromSourcePlaid {
            call_id: 9,
            event: EventSourcePlaid::PlaidFailed {
                item_id: "item-9".to_string(),
                stage: server_wire::SyncStage::Transactions,
                fault: src_plaid::PlaidFault::NotReady,
            },
        },

        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,
    );
}

/// D4a for somebody who has never linked a bank. Nothing running is an answer
/// and not a missing resource: they are told so and left connected, because a
/// bank landing a moment later is exactly what they are waiting for.
#[test]
fn a_user_with_nothing_syncing_is_told_so_and_kept() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SyncWatchRequested {
                conn_id: 1,
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(AL) },
        },

        > @r#"ToRouter { request_id: 1, action: Watching { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },

        // Still connected, so the sync that starts afterwards reaches them
        // without their asking again.
        < Event::FromStore {
            call_id: 98,
            event: EventStore::ItemSaved {
                item: Some(ItemFacts {
                    item_id: "item-9".to_string(),
                    user_id: AL,
                    cursor: None,
                    synced_at: None,
                    login_required: false,
                }),
            },
        },

        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: false } } }"#,

        > Action::ToSourcePlaid {
            action: ActionSourcePlaid::FetchBalances { .. },
            ..
        },
    );
}

/// A sync is pushed to the person whose bank it is and to nobody else. Two
/// accounts share a process and share no rows, and the owner rides on the
/// fetch rather than being looked up when there is somebody to tell.
#[test]
fn a_sync_reaches_only_the_user_whose_bank_it_is() {
    let mia = uuid::Uuid::from_u128(2);

    // Both pay. What is under test is whose sync reaches whom, and an
    // unsubscribed Mia would be kept out for the wrong reason.
    let mut state = state_subscribed_many(&[AL, mia]);

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SyncWatchRequested {
                conn_id: 1,
                token_sha256: hash_token("session-mia"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(mia) },
        },

        > @r#"ToRouter { request_id: 1, action: Watching { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },

        // Al's bank, Al's sync. Mia is watching and hears none of it.
        < Event::FromStore {
            call_id: 99,
            event: EventStore::ItemSaved {
                item: Some(ItemFacts {
                    item_id: "item-9".to_string(),
                    user_id: AL,
                    cursor: None,
                    synced_at: None,
                    login_required: false,
                }),
            },
        },

        > Action::ToSourcePlaid {
            action: ActionSourcePlaid::FetchBalances { .. },
            ..
        },
        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::BalancesLoaded {
                item_id: "item-9".to_string(),
                connection: Connection {
                    id: "item-9".to_string(),
                    name: "First Platypus Bank".to_string(),
                    initials: "FP".to_string(),
                    status: api_ledger::ConnectionStatus::Ok { synced_at: T0 },
                    accounts: vec![],
                },
            },
        },

        > Action::ToStore { action: ActionStore::SaveAccounts { .. }, .. },

        > Action::ToSourcePlaid {
            action: ActionSourcePlaid::SyncTransactions { .. },
            ..
        },
    );
}

/// A fetch that died is no longer running, so what was watching it is told the
/// sync is over. Dropped rather than marked done — a screen holding a stream
/// on a fetch that failed would otherwise sit on its last stage for good.
#[test]
fn a_failed_sync_stops_being_something_to_watch() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromStore {
            call_id: 99,
            event: EventStore::ItemSaved {
                item: Some(ItemFacts {
                    item_id: "item-9".to_string(),
                    user_id: AL,
                    cursor: None,
                    synced_at: None,
                    login_required: false,
                }),
            },
        },

        > Action::ToSourcePlaid {
            action: ActionSourcePlaid::FetchBalances { .. },
            ..
        },

        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SyncWatchRequested {
                conn_id: 1,
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::SessionResolved { user_id: Some(AL) },
        },

        > @r#"ToRouter { request_id: 1, action: Watching { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: false } } }"#,

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },

        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::PlaidFailed {
                item_id: "item-9".to_string(),
                stage: server_wire::SyncStage::Accounts,
                fault: src_plaid::PlaidFault::Failed(
                    "REQUEST_FAILED".to_string(),
                ),
            },
        },

        > @r#"ToSocket { action: Send { conn_id: 1, status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,
    );
}

/// A stream that has gone is not written to again. Which is what frees the
/// slot it was holding, so a phone that backgrounds all morning is not
/// eventually refused for the connections it already dropped.
#[test]
fn a_closed_stream_stops_being_written_to() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SyncWatchRequested {
                conn_id: 1,
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 1,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved { user_id: Some(AL) },
        },

        > Action::ToRouter { action: ActionRouter::Watching { .. }, .. },

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },

        < Event::FromSocket { event: EventSocket::Closed { conn_id: 1 } },

        < Event::FromStore {
            call_id: 99,
            event: EventStore::ItemSaved {
                item: Some(ItemFacts {
                    item_id: "item-9".to_string(),
                    user_id: AL,
                    cursor: None,
                    synced_at: None,
                    login_required: false,
                }),
            },
        },

        > Action::ToSourcePlaid {
            action: ActionSourcePlaid::FetchBalances { .. },
            ..
        },
    );
}

/// One caller may hold ten streams at once and not an eleventh. Refused
/// rather than closing the oldest: a client that cannot let go is a client
/// whose oldest connection is the one still working.
#[test]
fn an_eleventh_stream_is_refused() {
    let mut state = state_subscribed();

    // The ten it is allowed, opened the ordinary way. Not the claim, so they
    // are reached rather than written out.
    for conn_id in 1..=WATCHERS_AT_MOST as u64 {
        // Two calls each: the session read, and the audit row the
        // opened watch writes.
        let call_id = 2 * conn_id - 1;

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
        < Event::FromRouter {
            request_id: 11,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SyncWatchRequested {
                conn_id: 11,
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 21,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 21,
            event: EventStore::SessionResolved { user_id: Some(AL) },
        },

        > @r#"ToRouter { request_id: 11, action: TooMany }"#,

        // Letting one go frees the slot, which is what stops a phone that has
        // backgrounded all morning being locked out by connections it dropped.
        < Event::FromSocket { event: EventSocket::Closed { conn_id: 1 } },

        < Event::FromRouter {
            request_id: 12,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::SyncWatchRequested {
                conn_id: 12,
                token_sha256: hash_token("session-1"),
            },
        },

        > Action::ToStore {
            call_id: 22,
            action: ActionStore::LoadSession { .. },
        },
        < Event::FromStore {
            call_id: 22,
            event: EventStore::SessionResolved { user_id: Some(AL) },
        },

        > Action::ToRouter {
            action: ActionRouter::Watching { conn_id: 12, .. },
            ..
        },

        > Action::ToStore {
            action: ActionStore::RecordAccess { .. },
            ..
        },
    );
}

/// An expired login has to outlive the process, or the server comes back up
/// and asks Plaid the same question it was just refused. It is also what the
/// Reconnect button reads.
#[test]
fn an_expired_login_is_reported_and_written_down() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::PlaidFailed {
                item_id: "item-1".to_string(),
                stage: server_wire::SyncStage::Accounts,
                fault: src_plaid::PlaidFault::LoginRequired,
            },
        },

        > Action::ToProcess { action: ActionProcess::ReportLoginRequired },

        > Action::ToStore { action: ActionStore::SaveStatus { .. }, .. },
    );
}

/// Any other failure is just a failure: nothing to write down, and the next
/// request will try again.
#[test]
fn an_ordinary_failure_writes_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromSourcePlaid {
            call_id: 1,
            event: EventSourcePlaid::PlaidFailed {
                item_id: "item-1".to_string(),
                stage: server_wire::SyncStage::Accounts,
                fault: src_plaid::PlaidFault::Failed(
                    "INTERNAL_SERVER_ERROR".to_string(),
                ),
            },
        },
    );
}
