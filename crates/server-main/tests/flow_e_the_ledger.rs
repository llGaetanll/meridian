//! Flow E of `docs/API_FLOW.md` — into the app — tested the way
//! `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! E is one request, `GET /snapshot`, and it takes no parameters:
//! pull-to-refresh sends the exact same bytes as a launch, because when to go
//! back to a bank is the server's decision and not a gesture's. So most of
//! this file is `should_fetch` — the decision that turns one request into
//! either three calls to Plaid or a read out of Postgres.
//!
//! What it costs to get that wrong is why it is pinned this closely: Plaid
//! meters per endpoint, and a rejected call still counts against the budget.
//! The limit itself is `state_limits.rs`; here it is only the freshness
//! window and the dead login.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use api_ledger::Connection;
use fixtures::T0;
use fixtures::state_subscribed;
use fixtures::state_subscribed_many;

use server_main::*;

/// E. A launch, and the ledger — steps 1 and 2a.
///
/// The bank was synced a moment ago, so nothing is asked of Plaid and the
/// answer is the rows Postgres already holds. That is the ordinary shape of
/// this route: the expensive version is every other test in this file.
#[test]
fn a_launch_is_served_the_ledger_without_asking_plaid() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToStore { call_id: 3, action: ActionStore::LoadSnapshot { .. } },

        < Event::FromStore { call_id: 3, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// E2b. The session has run out, and the ledger is somebody's bank
/// transactions: there is no unauthenticated read here.
///
/// Decided in the machine rather than at the edge, which is what makes it a
/// test at all — and it is the same answer, on the same turn, for every route
/// that carries a token.
#[test]
fn a_token_that_names_nobody_is_served_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("stale") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: None } },
        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// A bank that has never synced is asked about immediately.
#[test]
fn a_bank_that_has_never_synced_is_fetched() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchBalances { .. } , ..},
    );
}

/// A user with no banks is served rather than fetched for. Not an error: it is
/// what "no bank linked yet" looks like, and the state first run exists to
/// leave.
#[test]
fn a_user_with_no_banks_is_just_served() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![] } },
        > Action::ToStore { call_id: 3, action: ActionStore::LoadSnapshot { .. } },

        < Event::FromStore { call_id: 3, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// Staleness is measured against the clock handed in, not one the machine
/// reads — which is what makes this a test rather than a wait.
#[test]
fn a_stale_bank_is_refetched_from_its_cursor() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: Some("cursor-1".to_string()), synced_at: Some(Timestamp::from_secs(T0.as_secs() - FRESH_FOR_SECS)), login_required: false }] } },
        > Action::ToSourcePlaid { call_id: 3, action: ActionSourcePlaid::FetchBalances { .. } },

        // The cursor is not on the call that starts a sync: the balances go
        // first, because they are the only place the bank's name exists. It
        // rides on the call it is for, one answer later.
        < Event::FromSourcePlaid { call_id: 3, event: EventSourcePlaid::BalancesLoaded { item_id: "item-1".to_string(), connection: Connection { id: "item-1".to_string(), name: "First Platypus Bank".to_string(), initials: "FP".to_string(), status: api_ledger::ConnectionStatus::Ok { synced_at: T0 }, accounts: vec![] } } },
        > Action::ToStore { call_id: 4, action: ActionStore::SaveAccounts { .. } },
        > @r#"ToSourcePlaid { call_id: 5, action: SyncTransactions { item_id: "item-1", cursor: Some("cursor-1") } }"#,
    );
}

/// The other half of the freshness window, and the reason a caller cannot
/// reach past it: asking again changes nothing. `/snapshot` used to take
/// `?refresh=true`, which made a pull-to-refresh a way to spend an
/// account-wide Plaid budget.
#[test]
fn asking_twice_over_does_not_refetch_a_fresh_bank() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToStore { call_id: 3, action: ActionStore::LoadSnapshot { .. } },

        < Event::FromStore { call_id: 3, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },

        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { call_id: 4, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 4, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { call_id: 5, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 5, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToStore { call_id: 6, action: ActionStore::LoadSnapshot { .. } },

        < Event::FromStore { call_id: 6, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// Retrying a login-required Item cannot help, and Plaid counts the call.
#[test]
fn a_login_required_bank_is_not_retried_however_stale() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: Some("cursor-1".to_string()), synced_at: Some(Timestamp::from_secs(T0.as_secs() - FRESH_FOR_SECS)), login_required: true }] } },
        > Action::ToStore { call_id: 3, action: ActionStore::LoadSnapshot { .. } },

        < Event::FromStore { call_id: 3, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// Every bank the user has is decided about separately, so one dead login does
/// not stop the others syncing.
#[test]
fn each_bank_is_decided_about_on_its_own() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }, ItemFacts { item_id: "item-2".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: true }] } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchBalances { .. } , ..},
    );
}

/// The route is answered once, after the last bank reports — not alongside the
/// fetches. It reads the store the moment it is told to, so being told earlier
/// would serve the very rows the sync was about to replace.
#[test]
fn a_snapshot_is_answered_after_its_last_bank() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }, ItemFacts { item_id: "item-2".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSourcePlaid { call_id: 3, action: ActionSourcePlaid::FetchBalances { .. } },
        > Action::ToSourcePlaid { call_id: 4, action: ActionSourcePlaid::FetchBalances { .. } },

        // One bank in. Still nothing to say: the other is outstanding.
        < Event::FromSourcePlaid { call_id: 3, event: EventSourcePlaid::PlaidFailed { item_id: "item-1".to_string(), stage: server_wire::SyncStage::Accounts, fault: src_plaid::PlaidFault::Failed("REQUEST_FAILED".to_string()) } },

        < Event::FromSourcePlaid { call_id: 4, event: EventSourcePlaid::PlaidFailed { item_id: "item-2".to_string(), stage: server_wire::SyncStage::Accounts, fault: src_plaid::PlaidFault::Failed("REQUEST_FAILED".to_string()) } },
        > Action::ToStore { call_id: 5, action: ActionStore::LoadSnapshot { .. } },

        < Event::FromStore { call_id: 5, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// A write that failed ends the wait like one that worked. The rows are the
/// ones we already had, which is a worse answer than a fresh sync and a far
/// better one than a request that never returns.
#[test]
fn a_failed_write_still_answers_the_route() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { call_id: 2, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSourcePlaid { call_id: 3, action: ActionSourcePlaid::FetchBalances { .. } },

        < Event::FromSourcePlaid { call_id: 3, event: EventSourcePlaid::BalancesLoaded { item_id: "item-1".to_string(), connection: Connection { id: "item-1".to_string(), name: "First Platypus Bank".to_string(), initials: "FP".to_string(), status: api_ledger::ConnectionStatus::Ok { synced_at: T0 }, accounts: vec![] } } },
        > Action::ToStore { call_id: 4, action: ActionStore::SaveAccounts { .. } },
        > Action::ToSourcePlaid { call_id: 5, action: ActionSourcePlaid::SyncTransactions { .. } },

        < Event::FromSourcePlaid { call_id: 5, event: EventSourcePlaid::TransactionsLoaded { item_id: "item-1".to_string(), added: vec![], modified: vec![], removed: vec![], cursor: "cursor-2".to_string(), has_more: false } },
        > Action::ToStore { call_id: 6, action: ActionStore::SaveTransactions { .. } },
        > Action::ToSourcePlaid { call_id: 7, action: ActionSourcePlaid::FetchRecurring { .. } },

        < Event::FromSourcePlaid { call_id: 7, event: EventSourcePlaid::RecurringLoaded { item_id: "item-1".to_string(), streams: vec![] } },
        > Action::ToStore { call_id: 8, action: ActionStore::SaveStreams { .. } },

        // Two of the three writes worked and the third did not, and the wait
        // ends on the last of them either way.
        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },

        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },

        < Event::FromStore { call_id: 8, event: EventStore::WriteError { message: "connection refused".to_string() } },
        > Action::ToStore { call_id: 9, action: ActionStore::LoadSnapshot { .. } },

        < Event::FromStore { call_id: 9, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// The whole point of the id. Two routes asking at once are answered on their
/// own ids, so a handler can find its own — before this, a batch was scanned
/// for the first matching action, which would hand somebody another person's
/// session token.
#[test]
fn two_routes_asking_at_once_get_their_own_answers() {
    // Both pay: what is under test is that two answers do not cross.
    let mut state = state_subscribed_many(&[uuid::Uuid::nil(), uuid::Uuid::from_u128(9)]);

    eval!(&mut state,
        < Event::FromRouter { request_id: 7, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-7") } },
        < Event::FromRouter { request_id: 9, client: Client { ip: ClientIp::new("198.51.100.4"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-9") } },
        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },
        > Action::ToStore { call_id: 2, action: ActionStore::LoadSession { .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        < Event::FromStore { call_id: 2, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::from_u128(9)) } },
        > Action::ToStore { call_id: 3, action: ActionStore::LoadItems { .. } },
        > Action::ToStore { call_id: 4, action: ActionStore::LoadItems { .. } },

        < Event::FromStore { call_id: 3, event: EventStore::ItemsLoaded { items: vec![] } },
        < Event::FromStore { call_id: 4, event: EventStore::ItemsLoaded { items: vec![] } },
        > Action::ToStore { call_id: 5, action: ActionStore::LoadSnapshot { .. } },
        > Action::ToStore { call_id: 6, action: ActionStore::LoadSnapshot { .. } },

        // Four hops each, interleaved the whole way, and each id comes back
        // to the route that drew it.
        < Event::FromStore { call_id: 5, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        < Event::FromStore { call_id: 6, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, request_id: 7 },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, request_id: 9 },
    );
}
