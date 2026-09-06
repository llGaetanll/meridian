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
use fixtures::T0;
use fixtures::state;

use server_main::*;

/// E. A launch, and the ledger — steps 1 and 2a.
///
/// The bank was synced a moment ago, so nothing is asked of Plaid and the
/// answer is the rows Postgres already holds. That is the ordinary shape of
/// this route: the expensive version is every other test in this file.
#[test]
fn a_launch_is_served_the_ledger_without_asking_plaid() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 3, .. } },

        < Event::FromStore { event: EventStore::SnapshotLoaded { call_id: 3, snapshot: WireSnapshot::default() } },
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
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("stale") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: None } },
        > @r#"ToRouter { request_id: 1, action: Unauthorized }"#,
    );
}

/// A bank that has never synced is asked about immediately.
#[test]
fn a_bank_that_has_never_synced_is_fetched() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: None, .. } },
    );
}

/// A user with no banks is served rather than fetched for. Not an error: it is
/// what "no bank linked yet" looks like, and the state first run exists to
/// leave.
#[test]
fn a_user_with_no_banks_is_just_served() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![] } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 3, .. } },

        < Event::FromStore { event: EventStore::SnapshotLoaded { call_id: 3, snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// Staleness is measured against the clock handed in, not one the machine
/// reads — which is what makes this a test rather than a wait.
#[test]
fn a_stale_bank_is_refetched_from_its_cursor() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(Timestamp::from_secs(T0.as_secs() - FRESH_FOR_SECS)), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: Some(_), .. } },
    );
}

/// The other half of the freshness window, and the reason a caller cannot
/// reach past it: asking again changes nothing. `/snapshot` used to take
/// `?refresh=true`, which made a pull-to-refresh a way to spend an
/// account-wide Plaid budget.
#[test]
fn asking_twice_over_does_not_refetch_a_fresh_bank() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 3, .. } },

        < Event::FromStore { event: EventStore::SnapshotLoaded { call_id: 3, snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },

        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 4, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 4, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 5, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 5, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 6, .. } },

        < Event::FromStore { event: EventStore::SnapshotLoaded { call_id: 6, snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// Retrying a login-required Item cannot help, and Plaid counts the call.
#[test]
fn a_login_required_bank_is_not_retried_however_stale() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(Timestamp::from_secs(T0.as_secs() - FRESH_FOR_SECS)), login_required: true }] } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 3, .. } },

        < Event::FromStore { event: EventStore::SnapshotLoaded { call_id: 3, snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// Every bank the user has is decided about separately, so one dead login does
/// not stop the others syncing.
#[test]
fn each_bank_is_decided_about_on_its_own() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }, ItemFacts { item_id: "item-2".to_string(), cursor: None, synced_at: None, login_required: true }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: None, .. } },
    );
}

/// The route is answered once, after the last bank reports — not alongside the
/// fetches. It reads the store the moment it is told to, so being told earlier
/// would serve the very rows the sync was about to replace.
#[test]
fn a_snapshot_is_answered_after_its_last_bank() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }, ItemFacts { item_id: "item-2".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 3, .. } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 4, .. } },

        // One bank in. Still nothing to say: the other is outstanding.
        < Event::FromSource { event: EventSource::PlaidFailed { call_id: 3, item_id: "item-1".to_string(), code: "REQUEST_FAILED".to_string() } },

        < Event::FromSource { event: EventSource::PlaidFailed { call_id: 4, item_id: "item-2".to_string(), code: "REQUEST_FAILED".to_string() } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 5, .. } },

        < Event::FromStore { event: EventStore::SnapshotLoaded { call_id: 5, snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// A write that failed ends the wait like one that worked. The rows are the
/// ones we already had, which is a worse answer than a fresh sync and a far
/// better one than a request that never returns.
#[test]
fn a_failed_write_still_answers_the_route() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 2, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 3, .. } },

        < Event::FromSource { event: EventSource::PlaidLoaded { call_id: 3, item_id: "item-1".to_string(), added: vec![], modified: vec![], removed: vec![], connection: None, streams: vec![], cursor: "cursor-2".into() } },
        > Action::ToStore { action: ActionStore::SaveFetched { call_id: 4, .. } },

        < Event::FromStore { event: EventStore::WriteError { call_id: 4, message: "connection refused".to_string() } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 5, .. } },

        < Event::FromStore { event: EventStore::SnapshotLoaded { call_id: 5, snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// The whole point of the id. Two routes asking at once are answered on their
/// own ids, so a handler can find its own — before this, a batch was scanned
/// for the first matching action, which would hand somebody another person's
/// session token.
#[test]
fn two_routes_asking_at_once_get_their_own_answers() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 7, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-7") } },
        < Event::FromRouter { request_id: 9, client: Client { ip: ClientIp::new("198.51.100.4"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-9") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::SessionResolved { call_id: 1, user_id: Some(uuid::Uuid::nil()) } },
        < Event::FromStore { event: EventStore::SessionResolved { call_id: 2, user_id: Some(uuid::Uuid::from_u128(9)) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 3, .. } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 4, .. } },

        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 3, items: vec![] } },
        < Event::FromStore { event: EventStore::ItemsLoaded { call_id: 4, items: vec![] } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 5, .. } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 6, .. } },

        // Four hops each, interleaved the whole way, and each id comes back
        // to the route that drew it.
        < Event::FromStore { event: EventStore::SnapshotLoaded { call_id: 5, snapshot: WireSnapshot::default() } },
        < Event::FromStore { event: EventStore::SnapshotLoaded { call_id: 6, snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, request_id: 7 },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, request_id: 9 },
    );
}
