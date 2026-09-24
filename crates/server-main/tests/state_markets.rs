//! Market data: the claims of issue #146, as event sequences.
//!
//! The heart of it is "never ask twice": a covered window asks Yahoo nothing,
//! an in-flight window seats a second caller on the first caller's fetch, and
//! a window a source refuses to serve is never asked for at all. Around that
//! sit the pacing rules — one fetch at a time, backed off on 429 — and the
//! promise that a route is held until its window is whole, but not forever.

mod fixtures;

use api_common::Bar;
use api_common::Timestamp;
use api_eval::eval;
use fixtures::T0;
use fixtures::state_subscribed;
use src_postgres::BarInterval;

use server_main::*;

const DAY: u64 = 86_400;

/// Ten days of daily bars, ending at `T0` — which sits on the daily grid, so
/// the settled edge cuts nothing off.
const START: Timestamp = Timestamp::from_secs(T0.as_secs() - 10 * DAY);

/// A window the store already knows is served from it, and Yahoo is asked
/// nothing — the actions below are all of them.
#[test]
fn a_covered_window_asks_yahoo_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BarsRequested {
                token_sha256: hash_token("session-1"),
                symbol: "SPY".to_string(),
                interval: BarInterval::Day1,
                start: START,
                end: T0,
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadBarCoverage { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::BarCoverageLoaded {
                spans: vec![START..T0],
            },
        },

        > Action::ToStore { call_id: 3, action: ActionStore::LoadBars { .. } },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::BarsLoaded {
                bars: vec![Bar {
                    ts: START,
                    open: None,
                    high: None,
                    low: None,
                    close: 100.0,
                    volume: 5,
                }],
            },
        },

        > Action::ToRouter {
            action: ActionRouter::ServeBars { bars },
            ..
        } => {
            assert_eq!(bars.symbol, "SPY");
            assert_eq!(bars.interval, "1d");
            assert_eq!(bars.bars.len(), 1);
        },
    );
}

/// A window nobody has fetched becomes one fetch, and the route is held
/// through the fetch *and its write*: the read it is answered from must find
/// the rows the fetch just paid for.
#[test]
fn a_missing_window_is_fetched_written_and_then_served() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BarsRequested {
                token_sha256: hash_token("session-1"),
                symbol: "SPY".to_string(),
                interval: BarInterval::Day1,
                start: START,
                end: T0,
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadBarCoverage { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::BarCoverageLoaded { spans: vec![] },
        },

        > Action::ToSourceYahoo {
            call_id: 3,
            action: ActionSourceYahoo::FetchChart { .. },
        },
        < Event::FromSourceYahoo {
            call_id: 3,
            event: EventSourceYahoo::ChartFetched {
                symbol: "SPY".to_string(),
                interval: BarInterval::Day1,
                bars: vec![Bar {
                    ts: START,
                    open: None,
                    high: None,
                    low: None,
                    close: 100.0,
                    volume: 5,
                }],
            },
        },

        > Action::ToStore { call_id: 4, action: ActionStore::SaveBars { .. } },
        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },

        > Action::ToStore { call_id: 5, action: ActionStore::LoadBars { .. } },
        < Event::FromStore {
            call_id: 5,
            event: EventStore::BarsLoaded {
                bars: vec![Bar {
                    ts: START,
                    open: None,
                    high: None,
                    low: None,
                    close: 100.0,
                    volume: 5,
                }],
            },
        },

        > Action::ToRouter { action: ActionRouter::ServeBars { .. }, .. },
    );
}

/// Two callers wanting overlapping windows cost Yahoo one trip: the second
/// is seated on the fetch the first started, and both are answered when it
/// lands.
#[test]
fn an_overlapping_ask_joins_the_fetch_already_out() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BarsRequested {
                token_sha256: hash_token("session-1"),
                symbol: "SPY".to_string(),
                interval: BarInterval::Day1,
                start: START,
                end: T0,
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadBarCoverage { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::BarCoverageLoaded { spans: vec![] },
        },

        > Action::ToSourceYahoo {
            call_id: 3,
            action: ActionSourceYahoo::FetchChart { .. },
        },

        // The second caller, same window. Its coverage read goes out and
        // comes back empty, and no second fetch follows: the one out is the
        // one it waits on.
        < Event::FromRouter {
            request_id: 2,
            client: Client { ip: ClientIp::new("203.0.113.8"), user: None },
            event: EventRouter::BarsRequested {
                token_sha256: hash_token("session-1"),
                symbol: "SPY".to_string(),
                interval: BarInterval::Day1,
                start: START,
                end: T0,
            },
        },

        > Action::ToStore { call_id: 4, action: ActionStore::LoadSession { .. } },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 5,
            action: ActionStore::LoadBarCoverage { .. },
        },
        < Event::FromStore {
            call_id: 5,
            event: EventStore::BarCoverageLoaded { spans: vec![] },
        },

        < Event::FromSourceYahoo {
            call_id: 3,
            event: EventSourceYahoo::ChartFetched {
                symbol: "SPY".to_string(),
                interval: BarInterval::Day1,
                bars: vec![],
            },
        },

        > Action::ToStore { call_id: 6, action: ActionStore::SaveBars { .. } },
        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },

        > Action::ToStore { call_id: 7, action: ActionStore::LoadBars { .. } },
        > Action::ToStore { call_id: 8, action: ActionStore::LoadBars { .. } },
        < Event::FromStore {
            call_id: 7,
            event: EventStore::BarsLoaded { bars: vec![] },
        },

        > Action::ToRouter {
            request_id: 1,
            action: ActionRouter::ServeBars { .. },
        },
        < Event::FromStore {
            call_id: 8,
            event: EventStore::BarsLoaded { bars: vec![] },
        },

        > Action::ToRouter {
            request_id: 2,
            action: ActionRouter::ServeBars { .. },
        },
    );
}

/// A 429 doubles the wait between sends, and holding until complete has a
/// deadline: once it passes, the route is answered with what the store
/// holds, and the fetch keeps going for whoever asks next.
#[test]
fn a_429_backs_off_and_the_deadline_answers_the_route() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BarsRequested {
                token_sha256: hash_token("session-1"),
                symbol: "SPY".to_string(),
                interval: BarInterval::Day1,
                start: START,
                end: T0,
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadBarCoverage { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::BarCoverageLoaded { spans: vec![] },
        },

        > Action::ToSourceYahoo {
            call_id: 3,
            action: ActionSourceYahoo::FetchChart { .. },
        },
        < Event::FromSourceYahoo {
            call_id: 3,
            event: EventSourceYahoo::ChartFailed {
                symbol: "SPY".to_string(),
                fault: YahooFault::RateLimited,
            },
        },

        // The pace started at 15 seconds and the 429 doubled it, so a tick
        // where the old pace would have sent again sends nothing.
        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(T0.as_secs() + PACE_START_SECS + 1),
            },
        },

        // Past the answer deadline and the doubled pace both: the route is
        // answered from the store, and the fetch goes out again — with
        // nobody seated on it.
        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(T0.as_secs() + 2 * PACE_START_SECS),
            },
        },

        > Action::ToStore { call_id: 4, action: ActionStore::LoadBars { .. } },
        > Action::ToSourceYahoo {
            call_id: 5,
            action: ActionSourceYahoo::FetchChart { .. },
        },
        < Event::FromStore {
            call_id: 4,
            event: EventStore::BarsLoaded { bars: vec![] },
        },

        > Action::ToRouter { action: ActionRouter::ServeBars { .. }, .. },
    );
}

/// A window the source's own documents say it will refuse is never asked
/// for: one-minute bars reach back seven days, so a week-old day of them is
/// answered from the store outright — empty, honestly.
#[test]
fn a_window_past_the_floor_is_not_asked_for() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BarsRequested {
                token_sha256: hash_token("session-1"),
                symbol: "SPY".to_string(),
                interval: BarInterval::Min1,
                start: START,
                end: Timestamp::from_secs(START.as_secs() + 3_600),
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadBarCoverage { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::BarCoverageLoaded { spans: vec![] },
        },

        > Action::ToStore { call_id: 3, action: ActionStore::LoadBars { .. } },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::BarsLoaded { bars: vec![] },
        },

        > Action::ToRouter { action: ActionRouter::ServeBars { .. }, .. },
    );
}

/// A window running backwards has no answer, and is refused before anything
/// is read for it.
#[test]
fn a_backwards_window_is_refused() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BarsRequested {
                token_sha256: hash_token("session-1"),
                symbol: "SPY".to_string(),
                interval: BarInterval::Day1,
                start: T0,
                end: T0,
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToRouter { action: ActionRouter::BadAsk, .. },
    );
}

/// A served request leaves nothing behind: no fetch, and no work the
/// machine invents for itself on a later tick. Fetching on the server's own
/// initiative is a decision that has not been designed yet, and until it is,
/// an idle server sends nothing.
#[test]
fn an_idle_tick_asks_for_nothing() {
    let mut state = state_subscribed();

    eval!(&mut state,
        < Event::FromRouter {
            request_id: 1,
            client: Client { ip: ClientIp::new("203.0.113.7"), user: None },
            event: EventRouter::BarsRequested {
                token_sha256: hash_token("session-1"),
                symbol: "SPY".to_string(),
                interval: BarInterval::Day1,
                start: START,
                end: T0,
            },
        },

        > Action::ToStore { call_id: 1, action: ActionStore::LoadSession { .. } },
        < Event::FromStore {
            call_id: 1,
            event: EventStore::SessionResolved {
                user_id: Some(uuid::Uuid::nil()),
            },
        },

        > Action::ToStore {
            call_id: 2,
            action: ActionStore::LoadBarCoverage { .. },
        },
        < Event::FromStore {
            call_id: 2,
            event: EventStore::BarCoverageLoaded {
                spans: vec![START..T0],
            },
        },

        > Action::ToStore { call_id: 3, action: ActionStore::LoadBars { .. } },
        < Event::FromStore {
            call_id: 3,
            event: EventStore::BarsLoaded { bars: vec![] },
        },

        > Action::ToRouter { action: ActionRouter::ServeBars { .. }, .. },

        < Event::FromProcess {
            event: EventProcess::Tick {
                now: Timestamp::from_secs(T0.as_secs() + 60),
            },
        },
    );
}
