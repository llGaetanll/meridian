//! Market data: which fetches to make, in what order, and how fast.
//!
//! A sub-machine the way [`StateSignup`](crate::StateSignup) is: it owns the
//! fetches in flight end to end, and [`StateServer`](crate::StateServer)
//! routes events here. What it decides is the whole of being smart about the
//! sources — a window already held is never asked for, a window being asked
//! for is never asked for twice, a request from a person goes out before one
//! the machine invented, and the pace adapts because the limits are nowhere
//! written down.
//!
//! What it does **not** hold is the data. Bars and coverage live in Postgres
//! and losing this machine's state costs only re-asking; the store's answers
//! arrive on events like everything else.

use std::collections::BTreeMap;
use std::ops::Range;

use api_common::Bar;
use api_common::Timestamp;
use src_postgres::BarInterval;
use src_postgres::BarSource;

use crate::Action;
use crate::ActionSourceYahoo;
use crate::ActionStore;
use crate::CallId;
use crate::MarketFetch;
use crate::MarketFetchStep;
use crate::MarketWaiter;
use crate::MarketWeight;
use crate::RequestId;
use crate::YahooFault;

/// How long to wait between sends when nothing has gone wrong yet.
///
/// Yahoo publishes no limit, so this is a guess in the polite direction; the
/// machine works toward the real one from here, a second at a time.
pub const PACE_START_SECS: u64 = 15;

/// The fastest the machine will ever send, however long it goes unpunished.
/// The limit is unstated, so the only proof of being under it is distance.
pub const PACE_FLOOR_SECS: u64 = 5;

/// The slowest, so a source answering 429 to everything is still probed
/// often enough to notice it relenting.
pub const PACE_CEILING_SECS: u64 = 600;

/// How many failures one fetch is allowed before its window is given up on.
/// A 429 is not a failure — it is the pace being wrong, and costs no attempt.
pub const FETCH_ATTEMPTS: u32 = 3;

/// How long a fetch may be out before it is treated as lost. The same
/// judgement as [`CALL_GOOD_FOR_SECS`](crate::CALL_GOOD_FOR_SECS), made here
/// because these calls have no route parked on them in [`Waiting`](crate::Waiting).
pub const FETCH_GOOD_FOR_SECS: u64 = 30;

/// How long a request is held for its fetches before being answered with
/// what is stored. Holding until complete is the promise; holding forever
/// against a source that keeps saying 429 is not.
pub const BARS_ANSWER_GOOD_FOR_SECS: u64 = 25;

/// Holes in a window narrower than this many bars are fetched as part of
/// their neighbours rather than as requests of their own: the bars cost far
/// less than another trip to a source that answers 429 when pushed.
pub const PLAN_SLACK_BARS: u64 = 8;

/// Past this many holes in one window, one request spanning the lot is
/// cheaper than filling them one by one.
pub const PLAN_SPANS_AT_MOST: usize = 3;

/// The most bars one `/bars` ask may name. Ten years of five-minute bars is
/// not a chart, and serving it would be a read nobody draws.
pub const BARS_ASKED_AT_MOST: u64 = 100_000;

/// How long a symbol rests after the machine plans it, before its coverage
/// is asked about again. The rest is what stops a fully fetched universe
/// being re-read from the store in a tight loop, and it costs nothing: a
/// person's request is never subject to it.
pub const PLANNED_REST_SECS: u64 = 600;

/// Market data fetching, from a window wanted to bars written down.
#[derive(Debug)]
pub struct StateMarkets {
    /// Every fetch in flight, at the step it has reached. The key is minted
    /// here: a fetch is not a route and not always a call, so neither id can
    /// name it for its whole life.
    fetches: BTreeMap<u64, MarketFetch>,

    /// The next fetch key. A counter, so a replay lands on the same numbers.
    next_fetch: u64,

    /// How long to wait between sends, learned: shorter after every answer,
    /// doubled on every 429.
    pace_secs: u64,

    /// When the last fetch went out. `None` has never sent, and may
    /// immediately.
    last_sent: Option<Timestamp>,

    /// The symbols the machine may fetch on its own initiative, each with
    /// its share of the idle budget. Read off a file at startup; empty is a
    /// machine that fetches only what users ask.
    universe: Vec<MarketWeight>,

    /// When each universe symbol was last planned, for the proportional
    /// share: the machine picks the largest `weight × time since planned`,
    /// so each symbol is chosen at a rate proportional to its weight and
    /// none starves. Bounded by the universe, whose keys nobody outside
    /// chooses.
    planned: BTreeMap<String, Timestamp>,

    /// The coverage read the machine has out for a plan of its own, if any:
    /// the call, the symbol and interval it is about, and when it went out.
    /// One at a time — the next candidate is picked once this one answers.
    generating: Option<(CallId, String, BarInterval, Timestamp)>,
}

impl Default for StateMarkets {
    fn default() -> Self {
        Self {
            fetches: BTreeMap::new(),
            next_fetch: 0,
            pace_secs: PACE_START_SECS,
            last_sent: None,
            universe: Vec::new(),
            planned: BTreeMap::new(),
            generating: None,
        }
    }
}

impl StateMarkets {
    /// What the process came up with: the universe file, parsed, or nothing.
    pub fn started(&mut self, universe: Vec<MarketWeight>) {
        self.universe = universe;
    }

    /// A request wants `want` of a series, and `covered` is what the store
    /// already knows. Whatever is missing becomes fetches — new ones, or a
    /// seat on ones already going — and the answer is how many the request
    /// now waits on. Zero is a window the store can serve outright.
    pub fn asked(
        &mut self,
        request_id: RequestId,
        symbol: &str,
        interval: BarInterval,
        want: Range<Timestamp>,
        covered: &[Range<Timestamp>],
        now: Timestamp,
    ) -> u32 {
        self.plan(Some(request_id), symbol, interval, want, covered, now);

        self.seated(request_id)
    }

    /// Turn a want into fetches: new ones for what nobody is getting, and —
    /// when there is a waiter — a seat on every one already going.
    fn plan(
        &mut self,
        waiter: Option<RequestId>,
        symbol: &str,
        interval: BarInterval,
        want: Range<Timestamp>,
        covered: &[Range<Timestamp>],
        now: Timestamp,
    ) {
        // Only what a fetch could settle is worth planning over: the bar
        // still forming is not a fact yet, and history past the source's
        // floor is a question with no answer.
        let want = clamp(&want, interval, now);

        let missing = subtract(&want, covered);
        let missing = coalesce(missing, interval);

        for span in missing {
            for span in self.join_or_queue(waiter, symbol, interval, span, now) {
                self.fetches.insert(
                    self.next_fetch,
                    MarketFetch {
                        symbol: symbol.to_string(),
                        interval,
                        start: span.start,
                        end: span.end,
                        waiters: waiter
                            .map(|request_id| MarketWaiter {
                                request_id,
                                since: now,
                            })
                            .into_iter()
                            .collect(),
                        attempts: 0,
                        step: MarketFetchStep::Queued,
                    },
                );

                self.next_fetch += 1;
            }
        }
    }

    /// Park the waiter on every existing fetch of this series its span
    /// overlaps, and hand back what no fetch is already getting.
    fn join_or_queue(
        &mut self,
        waiter: Option<RequestId>,
        symbol: &str,
        interval: BarInterval,
        span: Range<Timestamp>,
        now: Timestamp,
    ) -> Vec<Range<Timestamp>> {
        let mut going: Vec<Range<Timestamp>> = Vec::new();

        for fetch in self.fetches.values_mut() {
            if fetch.symbol != symbol || fetch.interval != interval {
                continue;
            }

            if fetch.start >= span.end || fetch.end <= span.start {
                continue;
            }

            going.push(fetch.start..fetch.end);

            if let Some(request_id) = waiter
                && !fetch.waiters.iter().any(|w| w.request_id == request_id)
            {
                fetch.waiters.push(MarketWaiter {
                    request_id,
                    since: now,
                });
            }
        }

        going.sort_by_key(|span| span.start);

        subtract(&span, &going)
    }

    /// How many fetches this request is currently seated on.
    fn seated(&self, request_id: RequestId) -> u32 {
        self.fetches
            .values()
            .filter(|fetch| fetch.waiters.iter().any(|w| w.request_id == request_id))
            .count() as u32
    }

    /// Whether the pace allows another send right now.
    pub fn may_send(&self, now: Timestamp) -> bool {
        // One at a time: a second ask sent while the first is unanswered is
        // sent blind, and blind is how a backoff overshoots.
        if self
            .fetches
            .values()
            .any(|fetch| matches!(fetch.step, MarketFetchStep::Out { .. }))
        {
            return false;
        }

        match self.last_sent {
            None => true,
            Some(sent) => now.since(sent).as_secs() >= self.pace_secs,
        }
    }

    /// The fetch to send next: one a person is waiting on before one the
    /// machine invented, oldest first within each. `None` is an empty queue.
    pub fn next_queued(&self) -> Option<u64> {
        self.fetches
            .iter()
            .filter(|(_, fetch)| matches!(fetch.step, MarketFetchStep::Queued))
            .min_by_key(|(id, fetch)| (fetch.waiters.is_empty(), **id))
            .map(|(id, _)| *id)
    }

    /// Send one queued fetch out on `call_id`.
    pub fn dispatch(
        &mut self,
        fetch_id: u64,
        call_id: CallId,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        let Some(fetch) = self.fetches.get_mut(&fetch_id) else {
            unreachable!("the id came from next_queued")
        };

        fetch.step = MarketFetchStep::Out {
            call_id,
            since: now,
        };
        self.last_sent = Some(now);

        actions.push(Action::ToSourceYahoo {
            call_id,
            action: ActionSourceYahoo::FetchChart {
                symbol: fetch.symbol.clone(),
                interval: fetch.interval,
                start: fetch.start,
                end: fetch.end,
            },
        });
    }

    /// Bars arrived for the fetch out on `call_id`; write them down.
    ///
    /// The write goes out on `save_call`, and the fetch holds its waiters
    /// until it lands. What the fetch settles is decided here: its window,
    /// cut at the last bar the clock says is complete — the source's word
    /// covers points, never knowledge.
    ///
    /// The routes released outright are the ones with nothing to wait for: a
    /// fetch the forming bar swallowed whole, with nothing to write.
    pub fn chart_fetched(
        &mut self,
        call_id: CallId,
        save_call: CallId,
        bars: &[Bar],
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) -> Vec<RequestId> {
        let Some(fetch_id) = self.out_on(call_id) else {
            tracing::warn!(call_id, "yahoo answered a fetch this machine is not making");

            return Vec::new();
        };

        // An answer is proof the pace is tolerated, so lean in a little.
        self.pace_secs = self.pace_secs.saturating_sub(1).max(PACE_FLOOR_SECS);

        let Some(fetch) = self.fetches.get_mut(&fetch_id) else {
            unreachable!("the id came from out_on")
        };

        let covered_start = fetch.start;
        let covered_end = fetch.end.min(interval_floor(fetch.interval, now));

        if bars.is_empty() && covered_start >= covered_end {
            let Some(fetch) = self.fetches.remove(&fetch_id) else {
                unreachable!("the entry was just read")
            };

            return waiters_of(fetch);
        }

        fetch.step = MarketFetchStep::Saving { call_id: save_call };

        actions.push(Action::ToStore {
            call_id: save_call,
            action: ActionStore::SaveBars {
                source: BarSource::Yahoo,
                symbol: fetch.symbol.clone(),
                interval: fetch.interval,
                bars: bars.to_vec(),
                covered_start,
                covered_end,
            },
        });

        Vec::new()
    }

    /// The fetch out on `call_id` served nothing, and this is what to do
    /// about each of the three ways. The routes released are answered with
    /// whatever the store holds.
    pub fn chart_failed(&mut self, call_id: CallId, fault: &YahooFault) -> Vec<RequestId> {
        let Some(fetch_id) = self.out_on(call_id) else {
            tracing::warn!(call_id, "yahoo refused a fetch this machine is not making");

            return Vec::new();
        };

        match fault {
            // The pace was wrong, not the ask: back off hard and requeue.
            // Waiters stay seated — the answer-by deadline is what stops a
            // long famine holding them forever.
            YahooFault::RateLimited => {
                self.pace_secs = (self.pace_secs * 2).min(PACE_CEILING_SECS);

                tracing::warn!(pace = self.pace_secs, "yahoo said 429; slowing down");

                let Some(fetch) = self.fetches.get_mut(&fetch_id) else {
                    unreachable!("the id came from out_on")
                };

                fetch.step = MarketFetchStep::Queued;

                Vec::new()
            }

            // Asking again buys the same answer, so the window is let go of.
            // Not remembered as a floor yet: the machine plans clear of the
            // documented floors already, and what else a source refuses is
            // not yet worth a table.
            YahooFault::Refused { code, description } => {
                let Some(fetch) = self.fetches.remove(&fetch_id) else {
                    unreachable!("the id came from out_on")
                };

                tracing::warn!(
                    symbol = fetch.symbol,
                    interval = fetch.interval.as_str(),
                    code,
                    description,
                    "yahoo would not serve a window; giving it up",
                );

                waiters_of(fetch)
            }

            YahooFault::Failed { message } => {
                let Some(fetch) = self.fetches.get_mut(&fetch_id) else {
                    unreachable!("the id came from out_on")
                };

                fetch.attempts += 1;

                if fetch.attempts < FETCH_ATTEMPTS {
                    tracing::warn!(
                        symbol = fetch.symbol,
                        attempts = fetch.attempts,
                        message,
                        "a fetch failed; queued again",
                    );

                    fetch.step = MarketFetchStep::Queued;

                    return Vec::new();
                }

                let Some(fetch) = self.fetches.remove(&fetch_id) else {
                    unreachable!("the entry was just read")
                };

                tracing::error!(
                    symbol = fetch.symbol,
                    interval = fetch.interval.as_str(),
                    message,
                    "a fetch kept failing; giving its window up",
                );

                waiters_of(fetch)
            }
        }
    }

    /// The store answered the write a fetch was holding its waiters for.
    /// `None` is a call that is not one of ours; an error releases the
    /// waiters too, because the window stays uncovered and holding them
    /// re-buys nothing — the next ask replans it.
    pub fn store_answered(&mut self, call_id: CallId, landed: bool) -> Option<Vec<RequestId>> {
        let fetch_id = self.fetches.iter().find_map(|(id, fetch)| {
            matches!(fetch.step, MarketFetchStep::Saving { call_id: saving } if saving == call_id)
                .then_some(*id)
        })?;

        let Some(fetch) = self.fetches.remove(&fetch_id) else {
            unreachable!("the id was just found")
        };

        if !landed {
            tracing::error!(
                symbol = fetch.symbol,
                "a fetch's write did not land; its window stays unknown",
            );
        }

        Some(waiters_of(fetch))
    }

    /// The series the machine should plan next on its own initiative, if it
    /// is time to plan one at all.
    ///
    /// Proportional share, deterministically: the candidate with the largest
    /// `weight × time since last planned` wins, so each symbol is picked at
    /// a rate proportional to its weight and even the lightest never
    /// starves. Nothing is picked while a person's work is queued, while a
    /// plan of the machine's own is already out, or for a symbol still
    /// resting.
    pub fn wants_plan(&self, now: Timestamp) -> Option<(String, BarInterval)> {
        if self.universe.is_empty() || self.generating.is_some() {
            return None;
        }

        // Only an idle turn is spent on the machine's own curiosity; a fetch
        // already out does not count, since planning does not send.
        if self
            .fetches
            .values()
            .any(|fetch| matches!(fetch.step, MarketFetchStep::Queued))
        {
            return None;
        }

        let candidate = self
            .universe
            .iter()
            .filter_map(|entry| {
                let planned = self.planned.get(&entry.symbol).copied();
                let rested = planned
                    .map(|at| now.since(at).as_secs())
                    .unwrap_or(u64::MAX);

                if rested < PLANNED_REST_SECS {
                    return None;
                }

                Some((entry.weight * rested as f64, &entry.symbol))
            })
            .max_by(|(a, a_symbol), (b, b_symbol)| {
                // Ties go to the lexicographically earlier symbol, so a
                // replay picks the same one.
                a.total_cmp(b).then_with(|| b_symbol.cmp(a_symbol))
            });

        // Daily bars only: they have no history floor, never expire from the
        // source, and cover the chart ranges that make a page feel instant.
        // Speculative intraday is poor value — it ages out of Yahoo in days —
        // so it stays demand-driven.
        candidate.map(|(_, symbol)| (symbol.clone(), BarInterval::Day1))
    }

    /// The machine is planning `symbol`, on the coverage read out on
    /// `call_id`. The symbol starts its rest now, failure included —
    /// otherwise a store that keeps refusing is a store asked every turn.
    pub fn planning(
        &mut self,
        symbol: String,
        interval: BarInterval,
        call_id: CallId,
        now: Timestamp,
    ) {
        debug_assert!(self.generating.is_none(), "one plan of our own at a time");

        self.planned.insert(symbol.clone(), now);
        self.generating = Some((call_id, symbol, interval, now));
    }

    /// The coverage a plan of the machine's own waited on. `None` is a call
    /// that is not ours. Whatever the symbol's history is missing becomes
    /// fetches with nobody seated on them, from the epoch to now — the
    /// planner clamps that to what is real.
    pub fn generation_planned(
        &mut self,
        call_id: CallId,
        covered: &[Range<Timestamp>],
        now: Timestamp,
    ) -> bool {
        let Some((generating, ..)) = self.generating else {
            return false;
        };

        if generating != call_id {
            return false;
        }

        let Some((_, symbol, interval, _)) = self.generating.take() else {
            unreachable!("the entry was just read")
        };

        self.plan(None, &symbol, interval, Timestamp::EPOCH..now, covered, now);

        true
    }

    /// A plan's coverage read failed. `None` is a call that is not ours; the
    /// symbol stays rested, and the machine moves on to the next candidate.
    pub fn generation_failed(&mut self, call_id: CallId) -> bool {
        match self.generating {
            Some((generating, ..)) if generating == call_id => {
                let Some((_, symbol, ..)) = self.generating.take() else {
                    unreachable!("the entry was just read")
                };

                tracing::warn!(symbol, "a plan's coverage read failed; moving on");

                true
            }
            _ => false,
        }
    }

    /// The tick's housekeeping: give up on fetches that have been out too
    /// long, and answer waiters that have waited long enough with what is
    /// stored.
    ///
    /// The routes handed back are to be answered from the store now.
    pub fn on_tick(&mut self, now: Timestamp) -> Vec<RequestId> {
        let mut released = Vec::new();

        // A fetch that has been out past all patience is treated as failed:
        // requeued, and given up on once its attempts are spent.
        let lost: Vec<u64> = self
            .fetches
            .iter()
            .filter(|(_, fetch)| {
                matches!(fetch.step, MarketFetchStep::Out { since, .. }
                    if now.since(since).as_secs() >= FETCH_GOOD_FOR_SECS)
            })
            .map(|(id, _)| *id)
            .collect();

        for fetch_id in lost {
            let Some(fetch) = self.fetches.get_mut(&fetch_id) else {
                unreachable!("the key came from the map")
            };

            fetch.attempts += 1;

            if fetch.attempts < FETCH_ATTEMPTS {
                tracing::warn!(symbol = fetch.symbol, "a fetch went quiet; queued again");

                fetch.step = MarketFetchStep::Queued;
            } else {
                let Some(fetch) = self.fetches.remove(&fetch_id) else {
                    unreachable!("the key came from the map")
                };

                tracing::error!(symbol = fetch.symbol, "a fetch kept going quiet; given up");

                released.extend(waiters_of(fetch));
            }
        }

        // Holding until complete has a deadline: past it, a route is answered
        // with what the store holds, and the fetch keeps going for the next
        // asker.
        for fetch in self.fetches.values_mut() {
            let (kept, over): (Vec<MarketWaiter>, Vec<MarketWaiter>) = fetch
                .waiters
                .drain(..)
                .partition(|waiter| now.since(waiter.since).as_secs() < BARS_ANSWER_GOOD_FOR_SECS);

            fetch.waiters = kept;
            released.extend(over.into_iter().map(|waiter| waiter.request_id));
        }

        // A coverage read of the machine's own that never answers would
        // block every later plan, so it is given up on like a lost fetch.
        // The symbol stays rested; nothing is owed to anybody.
        if let Some((_, _, _, since)) = self.generating
            && now.since(since).as_secs() >= FETCH_GOOD_FOR_SECS
        {
            let Some((_, symbol, ..)) = self.generating.take() else {
                unreachable!("the entry was just read")
            };

            tracing::warn!(symbol, "a plan's coverage read went quiet; moving on");
        }

        // One entry per seat, not per route: a route seated on two fetches
        // is released twice, and the count is what its answer waits on.
        released
    }

    /// The fetch out on a call, if any.
    fn out_on(&self, call_id: CallId) -> Option<u64> {
        self.fetches.iter().find_map(|(id, fetch)| {
            matches!(fetch.step, MarketFetchStep::Out { call_id: out, .. } if out == call_id)
                .then_some(*id)
        })
    }

    /// Fetches in flight, for the census.
    pub fn fetches(&self) -> usize {
        self.fetches.len()
    }
}

/// How many bars a window names, for refusing an ask nothing would draw.
pub fn bars_asked(interval: BarInterval, window: &Range<Timestamp>) -> u64 {
    window
        .end
        .since(window.start)
        .as_secs()
        .div_ceil(interval.seconds())
}

/// `want`, cut down to what a fetch could actually settle: nothing beyond
/// the last complete bar, and nothing past the source's floor.
fn clamp(want: &Range<Timestamp>, interval: BarInterval, now: Timestamp) -> Range<Timestamp> {
    let end = want.end.min(interval_floor(interval, now));

    let start = match history_floor(interval) {
        Some(depth) => want.start.max(now.saturating_sub_secs(depth)),
        None => want.start,
    };

    start..end
}

/// How far back Yahoo will serve an interval, in seconds, as its documents
/// state it — less a day of slack, because the cutoffs are enforced somewhere
/// past the letter of them and a refusal costs a fetch. `None` is history
/// with no floor.
fn history_floor(interval: BarInterval) -> Option<u64> {
    const DAY: u64 = 86_400;

    match interval {
        BarInterval::Min1 => Some(6 * DAY),
        BarInterval::Min2
        | BarInterval::Min5
        | BarInterval::Min15
        | BarInterval::Min30
        | BarInterval::Hour1 => Some(59 * DAY),
        BarInterval::Day1 | BarInterval::Week1 => None,
    }
}

/// The start of the bar `now` falls in — the settled edge of a series, past
/// which nothing is a fact yet.
fn interval_floor(interval: BarInterval, now: Timestamp) -> Timestamp {
    interval.floor(now)
}

/// The parts of `want` that `known` does not cover, in order. `known` must be
/// sorted and disjoint, which is how the store keeps it.
fn subtract(want: &Range<Timestamp>, known: &[Range<Timestamp>]) -> Vec<Range<Timestamp>> {
    let mut missing = Vec::new();
    let mut cursor = want.start;

    if want.start >= want.end {
        return missing;
    }

    for span in known {
        if span.end <= cursor || span.start >= want.end {
            continue;
        }

        if span.start > cursor {
            missing.push(cursor..span.start);
        }

        cursor = cursor.max(span.end);
    }

    if cursor < want.end {
        missing.push(cursor..want.end);
    }

    missing
}

/// Fewer, wider spans: holes almost touching are one ask, and a window shot
/// through with holes is fetched whole. Refetching a covered bar costs a row
/// already held; another round trip costs part of a budget with no stated
/// size.
fn coalesce(missing: Vec<Range<Timestamp>>, interval: BarInterval) -> Vec<Range<Timestamp>> {
    let slack = PLAN_SLACK_BARS * interval.seconds();

    let mut merged: Vec<Range<Timestamp>> = Vec::new();

    for span in missing {
        match merged.last_mut() {
            Some(last) if span.start.since(last.end).as_secs() < slack => {
                last.end = span.end;
            }
            _ => merged.push(span),
        }
    }

    if merged.len() > PLAN_SPANS_AT_MOST {
        let (Some(first), Some(last)) = (merged.first(), merged.last()) else {
            unreachable!("more than three spans is more than none")
        };

        return vec![first.start..last.end];
    }

    merged
}

/// A finished fetch's waiters, as the routes to answer.
fn waiters_of(fetch: MarketFetch) -> Vec<RequestId> {
    fetch
        .waiters
        .into_iter()
        .map(|waiter| waiter.request_id)
        .collect()
}
