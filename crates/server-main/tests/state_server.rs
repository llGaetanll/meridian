//! The server's state machine, tested the way `docs/STYLE.md` says: events in
//! with `<`, actions out with `>`.
//!
//! What these pin is when the server decides to go back to Plaid — the two
//! things that cost real money to get wrong are its per-endpoint rate limit
//! and retrying an Item whose login has expired — and that a sync's diff is
//! handed to the store rather than applied anywhere in here.
//!
//! The ledger itself is no longer the machine's, so it is no longer tested
//! here. `/transactions/sync` being a diff is now `src-postgres`'s
//! `apply_entries` to get right, and its tests are the ones that pin it.
//!
//! Every event is written out where it is sent rather than built by a helper,
//! so each test reads as the conversation it is. Three things recur in them:
//! time arrives as a `Tick` like any other fact from the edge, call ids are
//! handed out in order from 1 so an answer comes back on the one that asked,
//! and a route is released only once everything it started has been written —
//! which is what `EventStore::WriteSuccess` is doing on its own line.

use api_common::Timestamp;
use api_eval::eval;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Tx;

use server_main::*;

const T0: Timestamp = Timestamp::from_secs(1_787_097_600);

/// A machine that has been told what time it is and that its process is up —
/// the first two events the runtime sends, and neither of them any test's
/// claim. What it has *not* been told is a seed: the tests that mint anything
/// send one.
///
/// It collects Link sessions by polling, which is what a laptop does and what
/// every test here but the webhook ones is about.
fn state() -> StateServer {
    started(PlaidCollect::Poll)
}

/// The same machine, told that Plaid calls us instead.
///
/// The channel is a fact from the edge, on the event that says the process is
/// up, so this is the whole difference between the two — and the reason
/// `/link/finish` reads rather than polls is a decision made from it.
fn state_collecting_by_webhook() -> StateServer {
    started(PlaidCollect::Webhook)
}

fn started(collect: PlaidCollect) -> StateServer {
    let mut state = StateServer::new();

    state.handle_events(
        &[
            Event::FromProcess {
                event: EventProcess::Tick { now: T0 },
            },
            Event::FromProcess {
                event: EventProcess::Started { collect },
            },
        ],
        &mut Vec::new(),
    );

    state
}

/// A bank that has never synced is asked about immediately.
#[test]
fn a_bank_that_has_never_synced_is_fetched() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: false, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }] } },
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
        // Forced, which normally outranks everything short of a dead login.
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: true, items: vec![] } },
        > Action::ToRouter { action: ActionRouter::Serve, .. },
    );
}

#[test]
fn a_fresh_bank_is_served_without_asking_plaid_again() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: false, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToRouter { action: ActionRouter::Serve, .. },
    );
}

/// Staleness is measured against the clock handed in, not one the machine
/// reads — which is what makes this a test rather than a wait.
#[test]
fn a_stale_bank_is_refetched_from_its_cursor() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: false, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(Timestamp::from_secs(T0.as_secs() - FRESH_FOR_SECS)), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: Some(_), .. } },

    );
}

#[test]
fn a_forced_request_refetches_however_fresh() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: Some(_), .. } },

    );
}

/// Retrying a login-required Item cannot help, and Plaid counts the call.
#[test]
fn a_login_required_bank_is_not_retried_even_when_forced() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: true }] } },
        > Action::ToRouter { action: ActionRouter::Serve, .. },
    );
}

/// Every bank the user has is decided about separately, so one dead login does
/// not stop the others syncing.
#[test]
fn each_bank_is_decided_about_on_its_own() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: false, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }, ItemFacts { item_id: "item-2".to_string(), cursor: None, synced_at: None, login_required: true }] } },
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
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: false, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }, ItemFacts { item_id: "item-2".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 1, .. } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 2, .. } },

        // One bank in. Still nothing to say: the other is outstanding.
        < Event::FromSource { event: EventSource::PlaidFailed { call_id: 1, item_id: "item-1".to_string(), code: "REQUEST_FAILED".to_string() } },

        < Event::FromSource { event: EventSource::PlaidFailed { call_id: 2, item_id: "item-2".to_string(), code: "REQUEST_FAILED".to_string() } },
        > Action::ToRouter { action: ActionRouter::Serve, .. },
    );
}

/// A write that failed ends the wait like one that worked. The rows are the
/// ones we already had, which is a worse answer than a fresh sync and a far
/// better one than a request that never returns.
#[test]
fn a_failed_write_still_answers_the_route() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: false, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 1, .. } },

        < Event::FromSource { event: EventSource::PlaidLoaded { call_id: 1, item_id: "item-1".to_string(), added: vec![], modified: vec![], removed: vec![], connection: None, streams: vec![], cursor: "cursor-2".into() } },
        > Action::ToStore { action: ActionStore::SaveFetched { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::WriteError { call_id: 2, message: "connection refused".to_string() } },
        > Action::ToRouter { action: ActionRouter::Serve, .. },
    );
}

/// A source that goes quiet does not park a route forever.
///
/// The only thing in the machine that happens because *nothing* happened: no
/// answer means no event, so without a tick there is no turn in which to give
/// up. The route is served what the store already holds.
#[test]
fn a_call_that_never_answers_is_given_up_on() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: false, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 1, .. } },

        // Plaid says nothing. Time passes and nothing else arrives.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + 1) } },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + CALL_GOOD_FOR_SECS) } },
        > Action::ToRouter { action: ActionRouter::Serve, .. },
    );
}

/// A route that had no answer decided yet is told so, rather than left. Being
/// given up on before the machine knew what to say is the one case where there
/// is nothing held to release.
#[test]
fn a_route_with_nothing_decided_is_told_it_cannot_be_answered() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkRequested { user_id: uuid::Uuid::nil() } },
        > Action::ToSource { action: ActionSource::OpenLink { call_id: 1, .. } },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + CALL_GOOD_FOR_SECS) } },
        > Action::ToRouter { action: ActionRouter::Unavailable, .. },
    );
}

/// A tick with nothing outstanding decides nothing. Every second of a quiet
/// server goes through here, so it had better be free.
#[test]
fn a_tick_on_a_quiet_server_does_nothing() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + 3600) } },
    );
}

/// A sync goes to the store and nowhere else — and answers nobody, which is
/// what a sync on a timer will be. The machine holds no ledger to apply a diff
/// to, which is what retired the trap in `docs/PLAID.md` rather than guarding
/// against it.
#[test]
fn a_sync_is_handed_to_the_store() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::PlaidLoaded { call_id: 1, item_id: "item-1".to_string(), added: vec![Entry::from_tx("a".to_string(), Tx::Transfer { ts: Timestamp::from_secs(1_787_000_000), pos: Position { asset: "USD".into(), qty: -1.0 } })], modified: vec![], removed: vec![], connection: None, streams: vec![], cursor: "cursor-2".into() } },
        > Action::ToStore { action: ActionStore::SaveFetched { .. } },
    );
}

/// An expired login has to outlive the process, or the server comes back up
/// and asks Plaid the same question it was just refused.
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

/// Plaid meters per endpoint; a burst has to be refused rather than sent and
/// rejected, because a rejected call still counts against the budget.
#[test]
fn a_burst_of_forced_refreshes_is_rate_limited() {
    let mut state = state();

    // BalanceGet allows ten a minute, and each refresh here is its own route,
    // so the budget runs out in front of you: ten fetches, and then the
    // eleventh is answered from the store instead of asking Plaid again.
    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },

        < Event::FromRouter { request_id: 2, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },

        < Event::FromRouter { request_id: 3, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },

        < Event::FromRouter { request_id: 4, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },

        < Event::FromRouter { request_id: 5, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },

        < Event::FromRouter { request_id: 6, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },

        < Event::FromRouter { request_id: 7, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },

        < Event::FromRouter { request_id: 8, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },

        < Event::FromRouter { request_id: 9, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },

        < Event::FromRouter { request_id: 10, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },

        < Event::FromRouter { request_id: 11, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToRouter { action: ActionRouter::Serve, .. },
    );
}

/// The budget is per Item, so a second bank is not refused because the first
/// one spent its allowance.
#[test]
fn the_rate_limit_is_per_bank() {
    let mut state = state();

    // The first bank's ten a minute, spent. Which turn spends which is
    // `a_burst_of_forced_refreshes_is_rate_limited`'s claim, so the actions go
    // nowhere here.
    for request_id in 1..=10 {
        state.handle_events(
            &[Event::FromRouter {
                request_id,
                event: EventRouter::SnapshotRequested {
                    force: true,
                    items: vec![ItemFacts {
                        item_id: "item-1".to_string(),
                        cursor: Some("cursor-1".to_string()),
                        synced_at: Some(T0),
                        login_required: false,
                    }],
                },
            }],
            &mut Vec::new(),
        );
    }

    eval!(&mut state,
        < Event::FromRouter { request_id: 11, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-2".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. }, .. },
    );
}

#[test]
fn shutting_down_stops_new_work() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromTerminal { event: EventTerminal::SigInt },
        > Action::ToProcess { action: ActionProcess::Shutdown },

        < Event::FromRouter { request_id: 1, event: EventRouter::SnapshotRequested { force: true, items: vec![ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToRouter { action: ActionRouter::Serve, .. },
    );

    assert!(state.shutting_down());
}

/// A second signal while the first is still draining must not queue a second
/// shutdown.
#[test]
fn a_second_signal_is_ignored() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromTerminal { event: EventTerminal::SigInt },
        < Event::FromTerminal { event: EventTerminal::SigTerm },
        > Action::ToProcess { action: ActionProcess::Shutdown },
    );
}

/// A machine on its way out mints nothing. The generator belongs to the
/// running state and a drain has left it, so a sign-in arriving during a
/// shutdown is refused rather than half-finished by a process that is going
/// away — the same rule that stops a fetch.
#[test]
fn a_draining_machine_mints_nothing() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },
        < Event::FromTerminal { event: EventTerminal::SigInt },
        > Action::ToProcess { action: ActionProcess::Shutdown },

        < Event::FromRouter { request_id: 1, event: EventRouter::CodeRequested { email: "alex@example.com".to_string() } },
        > Action::ToRouter { action: ActionRouter::Unavailable, .. },
    );
}

/// Asking for a code stores its hash and sends the code, in that order and as
/// two actions: only one of them may ever hold the plaintext.
#[test]
fn asking_for_a_code_stores_a_hash_and_sends_the_code() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter { request_id: 1, event: EventRouter::CodeRequested { email: "alex@example.com".to_string() } },
        > Action::ToStore { action: ActionStore::SaveCode { call_id: 1, .. } },
        > Action::ToEmail { action: ActionEmail::SendCode { .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 1 } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// The whole reason the generator is in here: the same seed produces the same
/// code, so an event log replays to the value it produced the first time. Two
/// seeds, two codes, both written down — a test that only said "they differ"
/// would pass on a generator that had stopped being a replay of anything.
#[test]
fn the_same_seed_produces_the_same_code() {
    let mut seven = state();

    eval!(&mut seven,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter { request_id: 1, event: EventRouter::CodeRequested { email: "alex@example.com".to_string() } },
        > Action::ToStore { action: ActionStore::SaveCode { call_id: 1, .. } },
        > @r#"ToEmail { action: SendCode { email: "alex@example.com", code: "470977" } }"#,
    );

    let mut eight = state();

    eval!(&mut eight,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [8; 32] } },

        < Event::FromRouter { request_id: 1, event: EventRouter::CodeRequested { email: "alex@example.com".to_string() } },
        > Action::ToStore { action: ActionStore::SaveCode { call_id: 1, .. } },
        > @r#"ToEmail { action: SendCode { email: "alex@example.com", code: "751695" } }"#,
    );
}

/// No entropy, no credentials. A predictable sign-in code is a way into
/// somebody's bank, so a machine that is up but not yet seeded refuses rather
/// than filling in anything derivable — and this is the test that says nothing
/// is stored and nothing is sent.
///
/// It answers rather than going quiet. A route parks until its own action
/// comes back, so a refusal with no action is a request that never returns.
#[test]
fn an_unseeded_machine_will_not_mint_a_code() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::CodeRequested { email: "alex@example.com".to_string() } },
        > Action::ToRouter { action: ActionRouter::Unavailable, .. },
    );
}

#[test]
fn the_right_code_signs_you_in_and_spends_itself() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter { request_id: 1, event: EventRouter::CodeSubmitted { email: "alex@example.com".to_string(), code: "123456".to_string(), stored: Some(StoredCode { code_sha256: hash_code("123456"), expires_at: Timestamp::from_secs(T0.as_secs() + 600), attempts: 0 }) } },
        > Action::ToStore { action: ActionStore::CreateSession { call_id: 1, .. } },
        > Action::ToStore { action: ActionStore::DeleteCode { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 1 } },

        // Only now: a token that reaches the phone before its session row
        // exists is a sign-in that 401s on its own first request.
        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 2 } },
        > Action::ToRouter { action: ActionRouter::SignedIn { .. }, .. },
    );
}

/// A wrong code costs a guess and nothing else — no session, and the code is
/// still there to be got right.
#[test]
fn a_wrong_code_only_counts_a_guess() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter { request_id: 1, event: EventRouter::CodeSubmitted { email: "alex@example.com".to_string(), code: "654321".to_string(), stored: Some(StoredCode { code_sha256: hash_code("123456"), expires_at: Timestamp::from_secs(T0.as_secs() + 600), attempts: 0 }) } },
        > Action::ToStore { action: ActionStore::RecordAttempt { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 1 } },
        > Action::ToRouter { action: ActionRouter::Rejected, .. },
    );
}

/// An expired code counts a guess too. An attacker who could reset the cap by
/// waiting would have an unlimited number of tries at the next one.
#[test]
fn an_expired_code_is_rejected_and_still_counts() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter { request_id: 1, event: EventRouter::CodeSubmitted { email: "alex@example.com".to_string(), code: "123456".to_string(), stored: Some(StoredCode { code_sha256: hash_code("123456"), expires_at: Timestamp::from_secs(T0.as_secs()), attempts: 0 }) } },
        > Action::ToStore { action: ActionStore::RecordAttempt { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 1 } },
        > Action::ToRouter { action: ActionRouter::Rejected, .. },
    );
}

/// Out of guesses outranks a right answer, or the cap would be advice.
#[test]
fn an_exhausted_code_rejects_the_right_answer() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter { request_id: 1, event: EventRouter::CodeSubmitted { email: "alex@example.com".to_string(), code: "123456".to_string(), stored: Some(StoredCode { code_sha256: hash_code("123456"), expires_at: Timestamp::from_secs(T0.as_secs() + 600), attempts: 5 }) } },
        > Action::ToStore { action: ActionStore::RecordAttempt { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 1 } },
        > Action::ToRouter { action: ActionRouter::Rejected, .. },
    );
}

/// No code standing against the address. Rejected like any other, and
/// deliberately not distinguishable — the difference would say which addresses
/// have a sign-in in flight.
#[test]
fn an_address_with_no_code_is_rejected_the_same_way() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter { request_id: 1, event: EventRouter::CodeSubmitted { email: "alex@example.com".to_string(), code: "123456".to_string(), stored: None } },
        > Action::ToRouter { action: ActionRouter::Rejected, .. },
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
        < Event::FromRouter { request_id: 7, event: EventRouter::SnapshotRequested { force: false, items: vec![] } },
        < Event::FromRouter { request_id: 9, event: EventRouter::SnapshotRequested { force: false, items: vec![] } },
        > Action::ToRouter { action: ActionRouter::Serve, request_id: 7 },
        > Action::ToRouter { action: ActionRouter::Serve, request_id: 9 },
    );
}

/// Asking to connect a bank opens a session and nothing else. Nothing is
/// created at Plaid's end, so there is nothing yet to write down.
#[test]
fn connecting_a_bank_opens_a_link_session() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkRequested { user_id: uuid::Uuid::nil() } },
        > Action::ToSource { action: ActionSource::OpenLink { call_id: 1, .. } },
    );
}

/// The session is written down *before* its URL is handed over. The other
/// order leaves a window in which a session could be finished that we have no
/// record of asking for — which is what stops one account collecting a bank
/// another account connected.
#[test]
fn a_session_is_written_down_before_its_url_is_handed_over() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkRequested { user_id: uuid::Uuid::nil() } },
        > Action::ToSource { action: ActionSource::OpenLink { .. } },

        < Event::FromSource { event: EventSource::LinkOpened { call_id: 1, user_id: uuid::Uuid::nil(), link_token: "link-1".to_string(), hosted_link_url: "https://secure.plaid.com/x".to_string() } },
        > Action::ToStore { action: ActionStore::SaveLink { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 2 } },
        > Action::ToRouter { action: ActionRouter::LinkOpened { .. }, .. },
    );
}

/// A token is the answer whether or not the session says it has finished:
/// somebody partway through adding a second bank has produced a real Item
/// already.
#[test]
fn a_public_token_is_exchanged_even_before_the_session_finishes() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec!["link-1".to_string()], attached: 0 } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 1, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: false, public_tokens: vec!["public-1".to_string()] }] } },
        > Action::ToSource { action: ActionSource::ExchangePublicToken { .. } },
    );
}

/// Finished with nothing: they closed the sheet. An outcome, not a failure,
/// and the session is marked done so it is not polled forever.
#[test]
fn a_session_that_ends_with_nothing_is_collected_and_reported() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec!["link-1".to_string()], attached: 0 } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 1, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: true, public_tokens: vec![] }] } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 2 } },
        > Action::ToRouter { action: ActionRouter::Linked { count: 0 }, .. },
    );
}

/// Still open — somebody is still typing their password. Left pending, so the
/// next close asks again rather than giving up on a bank they did connect.
#[test]
fn an_open_session_is_left_pending() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec!["link-1".to_string()], attached: 0 } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 1, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: false, public_tokens: vec![] }] } },
        > Action::ToRouter { action: ActionRouter::Linked { count: 0 }, .. },
    );
}

/// The bank is stored and the session closed, in that order.
#[test]
fn a_linked_bank_is_stored_and_its_session_closed() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec!["link-1".to_string()], attached: 0 } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 1, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: false, public_tokens: vec!["public-1".to_string()] }] } },
        > Action::ToSource { action: ActionSource::ExchangePublicToken { call_id: 2, .. } },

        < Event::FromSource { event: EventSource::ItemLinked { call_id: 2, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 3, .. } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 4, .. } },
        > @r#"ToSource { action: FetchPlaid { call_id: 5, item_id: "item-9", cursor: None } }"#,

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 3 } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 4 } },
        > Action::ToRouter { action: ActionRouter::Linked { count: 1 }, .. },
    );
}

/// The route is answered as soon as the bank is attached, not when its history
/// has arrived. Those are different questions, and the first sync is the long
/// one — holding `/link/finish` open for it is the wait this removes.
///
/// So the fetch is on a call nobody is parked on: the two writes release the
/// route, and the fetch is still outstanding when it goes.
#[test]
fn a_new_bank_starts_syncing_without_holding_the_route() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec!["link-1".to_string()], attached: 0 } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 1, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: true, public_tokens: vec!["public-1".to_string()] }] } },
        > Action::ToSource { action: ActionSource::ExchangePublicToken { call_id: 2, .. } },

        < Event::FromSource { event: EventSource::ItemLinked { call_id: 2, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 3, .. } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 4, .. } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 5, .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 3 } },
        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 4 } },
        > Action::ToRouter { action: ActionRouter::Linked { count: 1 }, .. },
    );
}

/// A sync is something to watch from the moment it is asked for, not from the
/// moment its first stage lands.
///
/// The gap between those two used to be a real one, and it cost a screen: the
/// phone asked how the sync was going in the instant after `/link/finish`
/// answered, was told nothing was running — which reads identically to "it
/// finished" — and decided the sync was over before Plaid had been called.
#[test]
fn a_bank_is_syncing_the_moment_the_fetch_is_asked_for() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec!["link-1".to_string()], attached: 0 } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 1, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: true, public_tokens: vec!["public-1".to_string()] }] } },
        > Action::ToSource { action: ActionSource::ExchangePublicToken { call_id: 2, .. } },

        < Event::FromSource { event: EventSource::ItemLinked { call_id: 2, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 3, .. } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 4, .. } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 5, .. } },

        // Asked in the turn above and not yet begun — and already something to
        // watch. No progress has been reported, so the counts are zero, but
        // `finished` is what the screen reads and it says no.
        < Event::FromRouter { request_id: 2, event: EventRouter::SyncStatusRequested { items: vec![ItemFacts { item_id: "item-9".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > @r#"ToRouter { request_id: 2, action: SyncStatus { status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: false } } }"#,
    );
}

/// Nothing running is an answer, not a missing resource: a user who has never
/// linked a bank gets `finished` and zeroes rather than a 404.
#[test]
fn a_user_with_nothing_syncing_is_told_so() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::SyncStatusRequested { items: vec![] } },
        > @r#"ToRouter { request_id: 1, action: SyncStatus { status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,
    );
}

/// Progress is kept as it arrives and read back against the asking user's own
/// banks. It is the one thing here that exists only to be looked at, and it is
/// kept because it cannot be recovered — the fetch is over by the time
/// anything else could go and read what it did.
#[test]
fn a_running_sync_reports_the_stage_it_is_on() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::PlaidProgressed { item_id: "item-9".to_string(), stage: server_wire::SyncStage::Transactions, institution: Some("First Platypus Bank".to_string()), accounts: 8, transactions: 19, streams: 0 } },

        < Event::FromRouter { request_id: 1, event: EventRouter::SyncStatusRequested { items: vec![ItemFacts { item_id: "item-9".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > @r#"ToRouter { request_id: 1, action: SyncStatus { status: WireSyncStatus { stage: "transactions", institution: Some("First Platypus Bank"), accounts: 8, transactions: 19, streams: 0, finished: false } } }"#,
    );
}

/// A fetch that has answered is no longer running, so what was watching it is
/// told the sync is over. Dropped on a failure too — a screen polling a fetch
/// that died would otherwise sit on its last stage for good.
#[test]
fn a_failed_sync_stops_being_something_to_watch() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::PlaidProgressed { item_id: "item-9".to_string(), stage: server_wire::SyncStage::Transactions, institution: Some("First Platypus Bank".to_string()), accounts: 8, transactions: 19, streams: 0 } },

        < Event::FromSource { event: EventSource::PlaidFailed { call_id: 1, item_id: "item-9".to_string(), code: "REQUEST_FAILED".to_string() } },

        < Event::FromRouter { request_id: 1, event: EventRouter::SyncStatusRequested { items: vec![ItemFacts { item_id: "item-9".to_string(), cursor: None, synced_at: None, login_required: false }] } },
        > @r#"ToRouter { request_id: 1, action: SyncStatus { status: WireSyncStatus { stage: "accounts", institution: None, accounts: 0, transactions: 0, streams: 0, finished: true } } }"#,
    );
}

/// Closing the sheet with nothing open asks Plaid nothing. There is no session
/// to collect, so there is no call worth making.
#[test]
fn closing_a_sheet_with_no_session_open_asks_nothing() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec![], attached: 0 } },
        > Action::ToRouter { action: ActionRouter::Linked { count: 0 }, .. },
    );
}

/// Every session the user has open is asked about, so somebody who backed out
/// of one bank and connected another does not lose the one that worked.
#[test]
fn every_open_session_is_polled() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec!["link-1".to_string(), "link-2".to_string()], attached: 0 } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },
    );
}

/// One question, one answer. The route can only send one response, so however
/// many sessions were asked about, exactly one action comes back for it — the
/// invariant the whole request/reply pairing rests on.
#[test]
fn many_sessions_still_produce_one_answer() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec!["link-1".to_string(), "link-2".to_string(), "link-3".to_string()], attached: 0 } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 1, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: true, public_tokens: vec![] }, LinkPoll { link_token: "link-2".to_string(), finished: true, public_tokens: vec![] }, LinkPoll { link_token: "link-3".to_string(), finished: true, public_tokens: vec![] }] } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 2, .. } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 3, .. } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 4, .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 2 } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 3 } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 4 } },
        > Action::ToRouter { action: ActionRouter::Linked { count: 0 }, .. },
    );
}

/// The webhook doing what polling does: the token arrives on its own, and the
/// bank is attached from it. Everything after the exchange is the same path a
/// poll takes, which is the point — there is one way to attach a bank.
#[test]
fn a_finished_session_webhook_exchanges_its_token() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::WebhookReceived { webhook: WebhookFacts::SessionFinished { link_token: "link-1".to_string(), public_tokens: vec!["public-1".to_string()] } } },
        > @r#"ToSource { action: ExchangePublicToken { call_id: 1, link_token: "link-1", public_token: "public-1" } }"#,

        < Event::FromSource { event: EventSource::ItemLinked { call_id: 1, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 2, .. } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 3, .. } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 4, .. } },
    );
}

/// One webhook can carry two banks, and there is no second call to collect the
/// one that was left — Plaid says this once.
#[test]
fn every_token_a_session_produced_is_exchanged() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::WebhookReceived { webhook: WebhookFacts::SessionFinished { link_token: "link-1".to_string(), public_tokens: vec!["public-1".to_string(), "public-2".to_string()] } } },
        > @r#"ToSource { action: ExchangePublicToken { call_id: 1, link_token: "link-1", public_token: "public-1" } }"#,
        > @r#"ToSource { action: ExchangePublicToken { call_id: 2, link_token: "link-1", public_token: "public-2" } }"#,
    );
}

/// They closed the sheet. An outcome, not a failure — and the session is
/// marked done so nothing goes back to it.
#[test]
fn a_session_webhook_with_no_token_closes_the_session() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::WebhookReceived { webhook: WebhookFacts::SessionFinished { link_token: "link-1".to_string(), public_tokens: vec![] } } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 1 } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// Plaid saying there is something new is not a freshness question, so a bank
/// synced a second ago is asked about anyway.
#[test]
fn a_sync_webhook_fetches_however_fresh_the_bank_is() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::WebhookReceived { webhook: WebhookFacts::SyncUpdatesAvailable { item_id: "item-1".to_string(), item: Some(ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }) } } },
        > @r#"ToSource { action: FetchPlaid { call_id: 1, item_id: "item-1", cursor: Some("cursor-1") } }"#,
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// Nothing is parked on the fetch. A first sync of a long history takes longer
/// than Plaid waits for its 2xx, and a webhook Plaid decides went unanswered
/// is one it sends again.
#[test]
fn a_sync_webhook_is_answered_before_its_fetch_lands() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::WebhookReceived { webhook: WebhookFacts::SyncUpdatesAvailable { item_id: "item-1".to_string(), item: Some(ItemFacts { item_id: "item-1".to_string(), cursor: None, synced_at: None, login_required: false }) } } },
        > Action::ToSource { action: ActionSource::FetchPlaid { .. } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// No amount of asking fixes a login, and a webhook does not change that.
#[test]
fn a_sync_webhook_does_not_retry_a_dead_login() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::WebhookReceived { webhook: WebhookFacts::SyncUpdatesAvailable { item_id: "item-1".to_string(), item: Some(ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: true }) } } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// Plaid still has news about a bank the store has no row for. There is
/// nothing to sync against, and the webhook is taken anyway — refusing it
/// would only make Plaid send it again.
#[test]
fn a_sync_webhook_for_a_bank_we_do_not_hold_is_still_taken() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::WebhookReceived { webhook: WebhookFacts::SyncUpdatesAvailable { item_id: "item-gone".to_string(), item: None } } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// The write is what makes it survive a restart: a server that came back up
/// having forgotten would ask Plaid the same question and be refused the same
/// way. It is also what the Reconnect button reads.
#[test]
fn a_dead_login_webhook_is_written_down() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::WebhookReceived { webhook: WebhookFacts::Login { item_id: "item-1".to_string(), required: true } } },
        > Action::ToProcess { action: ActionProcess::ReportLoginRequired },
        > @r#"ToStore { action: SaveStatus { call_id: 1, item_id: "item-1", error_code: Some("ITEM_LOGIN_REQUIRED") } }"#,

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 1 } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// They signed in again. The only thing that clears the flag without a sync
/// being tried, which is what makes the connection usable again immediately.
#[test]
fn a_repaired_login_webhook_clears_it() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::WebhookReceived { webhook: WebhookFacts::Login { item_id: "item-1".to_string(), required: false } } },
        > @r#"ToStore { action: SaveStatus { call_id: 1, item_id: "item-1", error_code: None } }"#,
    );
}

/// Plaid sends types nobody planned for and retries anything not answered 2xx,
/// so an unfamiliar one is taken and nothing is done about it.
#[test]
fn an_unknown_webhook_is_taken_and_acted_on_by_nothing() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::WebhookReceived { webhook: WebhookFacts::Ignored { webhook_type: "ASSETS".to_string(), webhook_code: "PRODUCT_READY".to_string() } } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// With Plaid calling us, a closed sheet is a read of what already arrived —
/// no `PollLinks`, which is the whole claim. The sessions still standing are
/// marked done, because nothing else ever revisits one.
#[test]
fn a_closed_sheet_is_read_rather_than_polled_when_plaid_calls_us() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec!["link-1".to_string()], attached: 1 } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 1, .. } },

        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 1 } },
        > Action::ToRouter { action: ActionRouter::Linked { count: 1 }, .. },
    );
}

/// The webhook has not arrived yet, so nothing has been attached and the
/// answer says so. Zero is what somebody who closed the sheet gets too — the
/// route has no way to tell those apart, and the bank still lands when the
/// webhook does.
#[test]
fn a_closed_sheet_with_nothing_attached_answers_zero() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec![], attached: 0 } },
        > Action::ToRouter { action: ActionRouter::Linked { count: 0 }, .. },
    );
}

/// The laptop cannot receive a webhook, so polling has to be untouched by all
/// of the above: the same close, on a machine told to poll, still asks Plaid.
#[test]
fn a_closed_sheet_still_polls_when_we_collect_by_polling() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, event: EventRouter::LinkClosed { pending: vec!["link-1".to_string()], attached: 1 } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },
    );
}
