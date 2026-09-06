//! The machine's own life — ticks, sweeps, timeouts and shutdown — tested the
//! way `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! None of this is a flow in `docs/API_FLOW.md`, because none of it is
//! anybody's request. It is the two things that happen for reasons outside
//! every flow: a clock that moves, and a signal that says stop. A tick is the
//! only way the machine learns what time it is, and the only thing in here
//! that happens because *nothing* happened — no answer means no event, so
//! without a tick there is no turn in which to give up on a source that has
//! gone quiet.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use fixtures::T0;
use fixtures::state;

use server_main::*;

/// A tick with nothing outstanding decides nothing. Every second of a quiet
/// server goes through here, so it had better be free.
#[test]
fn a_tick_on_a_quiet_server_does_nothing() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + 1) } },
    );
}

/// Once an hour a quiet server does decide one thing: the expired sessions go.
/// Nobody is waiting on it — an expired session already signs nobody in — and
/// it is the machine that asks, so the same log replays the same sweeps (#24).
#[test]
fn expired_sessions_are_swept_on_the_hour() {
    let mut state = state();

    eval!(&mut state,
        // Not yet. The clock is what decides, and a second short is short.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + SWEEP_SESSIONS_EVERY_SECS - 1) } },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + SWEEP_SESSIONS_EVERY_SECS) } },
        > Action::ToStore { action: ActionStore::SweepSessions { .. } },

        // And not again on the tick after it.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + SWEEP_SESSIONS_EVERY_SECS + 1) } },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + 2 * SWEEP_SESSIONS_EVERY_SECS) } },
        > Action::ToStore { action: ActionStore::SweepSessions { .. } },
    );
}

/// A machine on its way out sweeps nothing. It writes what is outstanding and
/// starts nothing new, and a table that grows for the seconds a process takes
/// to stop grows by nothing.
#[test]
fn a_stopping_server_does_not_sweep() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromTerminal { event: EventTerminal::SigInt },
        > Action::ToProcess { action: ActionProcess::Shutdown },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + SWEEP_SESSIONS_EVERY_SECS) } },
    );
}

/// A source that goes quiet does not park a route forever. The route is served
/// what the store already holds, and `handle_timeouts` is the only thing that
/// gets it there.
#[test]
fn a_call_that_never_answers_is_given_up_on() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToSource { action: ActionSource::FetchPlaid { call_id: 3, .. } },

        // Plaid says nothing. Time passes and nothing else arrives.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + 1) } },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + CALL_GOOD_FOR_SECS) } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 4, .. } },

        < Event::FromStore { call_id: 4, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );
}

/// A route that had no answer decided yet is told so, rather than left. Being
/// given up on before the machine knew what to say is the one case where there
/// is nothing held to release.
#[test]
fn a_route_with_nothing_decided_is_told_it_cannot_be_answered() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToSource { action: ActionSource::OpenLink { call_id: 2, .. } },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + CALL_GOOD_FOR_SECS) } },
        > Action::ToRouter { action: ActionRouter::Unavailable, .. },
    );
}

/// A signal on an idle server stops it in the turn it arrives in.
///
/// `Stopping` is transient. Nothing is outstanding here, so there is nothing
/// to wait for and the machine says so at once — which is what the loop is
/// watching for.
#[test]
fn a_signal_on_an_idle_server_stops_it_at_once() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromTerminal { event: EventTerminal::SigInt },
        > Action::ToProcess { action: ActionProcess::Shutdown },
    );

    assert!(state.stopped(), "nothing was outstanding");
}

/// A signal while a call is out waits for it, and stops on the turn it lands.
///
/// The whole reason the machine says when rather than the runtime guessing:
/// only the machine knows what it is still owed. Stopping on the signal would
/// drop a write that had already been asked for.
#[test]
fn a_signal_with_a_call_out_waits_for_it() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::CodeRequested { email: "alex@example.com".to_string() } },
        > Action::ToStore { action: ActionStore::SaveCode { call_id: 1, .. } },
        > Action::ToEmail { action: ActionEmail::SendCode { .. } },

        < Event::FromTerminal { event: EventTerminal::SigInt },
        > Action::ToProcess { action: ActionProcess::Shutdown },
    );

    assert!(!state.stopped(), "a write is still out");

    eval!(&mut state,
        < Event::FromStore { call_id: 1, event: EventStore::WriteSuccess },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );

    assert!(
        state.stopped(),
        "the write landed and the route was answered"
    );
}

/// A call that never comes back does not hold the shutdown open forever.
///
/// `handle_timeouts` is what bounds it: the route is answered anyway and the
/// machine is free to stop. Without this a source that went quiet would be a
/// process that never exits.
#[test]
fn a_shutdown_does_not_wait_forever_on_a_quiet_source() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromProcess { event: EventProcess::Seeded { seed: [7; 32] } },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::CodeRequested { email: "alex@example.com".to_string() } },
        > Action::ToStore { action: ActionStore::SaveCode { call_id: 1, .. } },
        > Action::ToEmail { action: ActionEmail::SendCode { .. } },

        < Event::FromTerminal { event: EventTerminal::SigInt },
        > Action::ToProcess { action: ActionProcess::Shutdown },

        // The store never answers. Time passes and the route is let go of.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + CALL_GOOD_FOR_SECS) } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );

    assert!(state.stopped(), "the call was given up on");
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

/// A machine on its way out serves what it can and starts nothing new. The
/// request still gets its ledger — everything it needs is already in the store
/// — and no fetch goes out for the bank that has never synced.
#[test]
fn shutting_down_stops_new_work() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromTerminal { event: EventTerminal::SigInt },
        > Action::ToProcess { action: ActionProcess::Shutdown },

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-1".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }] } },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 3, .. } },

        < Event::FromStore { call_id: 3, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { action: ActionRouter::Serve { .. }, .. },
    );

    assert!(state.shutting_down());
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

        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::CodeRequested { email: "alex@example.com".to_string() } },
        > Action::ToRouter { action: ActionRouter::Unavailable, .. },
    );
}
