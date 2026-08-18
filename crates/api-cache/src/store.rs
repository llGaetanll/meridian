use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use api_common::Timestamp;
use futures::channel::oneshot;

use crate::Bars;
use crate::Coverage;
use crate::Fetched;
use crate::Interval;
use crate::Source;

/// Everything the cache decides logs under this target, so `target=cache`
/// answers "why did that request go out, or not".
const TARGET: &str = "cache";

/// Holes narrower than this many bars are not worth their own request. The
/// bars themselves cost far less than another round trip to an API that
/// answers 429 when pushed.
const GAP_SLACK_BARS: u64 = 8;

/// Past this many holes in one window, a single request covering the lot is
/// the cheaper way to fill them.
const MAX_REQUESTS: usize = 3;

/// Bars kept by symbol and interval, fetched only where they are not already
/// held.
///
/// The store is asked for a window rather than a range, which is what lets it
/// answer one from pieces of others: ask for `t0..t1`, then `t1..t2`, then
/// `t0..t2`, and the third asks nobody anything. What it holds is what a
/// source served, at the interval it served — nothing is thinned or squashed
/// on the way in.
///
/// Nothing here reaches for a clock, a filesystem or a network. It is handed a
/// source and told which windows to hold, so it runs the same in a browser, in
/// a desktop shell, or on a phone.
pub struct Store<S: Source> {
    source: Rc<S>,
    inner: Rc<RefCell<Inner<S::Snapshot>>>,
}

struct Inner<Snapshot> {
    series: HashMap<Key, Entry>,

    snapshots: HashMap<String, Snapshot>,

    /// What a source really serves when asked for a given interval, learned
    /// the first time it substitutes one. Without this, every request for the
    /// interval it will not serve goes out again and lands in the same other
    /// place.
    substitutes: HashMap<Key, Interval>,

    /// Who to tell when the fetch under way for a key is done. A key present
    /// here is a key being fetched.
    waiting: HashMap<Key, Vec<oneshot::Sender<()>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    symbol: String,
    interval: Interval,
}

#[derive(Default)]
struct Entry {
    bars: Bars,
    coverage: Coverage,
}

impl<S: Source> Store<S> {
    pub fn new(source: S) -> Self {
        Self {
            source: Rc::new(source),
            inner: Rc::new(RefCell::new(Inner {
                series: HashMap::new(),
                snapshots: HashMap::new(),
                substitutes: HashMap::new(),
                waiting: HashMap::new(),
            })),
        }
    }

    /// The bars for `symbol` over `window`, fetching whatever part of it is
    /// not held yet.
    ///
    /// An error means the fetch failed; whatever was already held is still
    /// returned by [`Self::held`].
    pub async fn bars(
        &self,
        symbol: &str,
        interval: Interval,
        window: Range<Timestamp>,
    ) -> Result<Bars, String> {
        let key = Key {
            symbol: symbol.to_string(),
            interval,
        };

        let result = self.fill(&key, &window).await;
        let bars = self.held(symbol, interval, &window);

        // Data served from the cache is still data the app acted on. A log
        // showing only the requests that went out would leave most of a
        // session unaccounted for, so every answer is logged, cached or not.
        tracing::info!(
            target: TARGET,
            symbol,
            interval = ?interval,
            window = %format_args!("{}..{}", window.start, window.end),
            bars = bars.len(),
            cached = result.as_ref().is_ok_and(|&fetched| !fetched),
            "served",
        );

        // A window already answered in full is worth more than the error from
        // a fetch that need not have gone out.
        match result {
            Err(error) if bars.is_empty() => Err(error),
            _ => Ok(bars),
        }
    }

    /// What is held for this window right now, without asking anyone.
    pub fn held(&self, symbol: &str, interval: Interval, window: &Range<Timestamp>) -> Bars {
        let inner = self.inner.borrow();

        let key = inner.resolve(&Key {
            symbol: symbol.to_string(),
            interval,
        });

        inner
            .series
            .get(&key)
            .map(|entry| entry.bars.window(window))
            .unwrap_or_default()
    }

    /// The last thing a source said about the symbol itself.
    pub fn snapshot(&self, symbol: &str) -> Option<S::Snapshot> {
        self.inner.borrow().snapshots.get(symbol).cloned()
    }

    /// Fetch what `window` is missing, once. A caller that finds a fetch
    /// already under way for the same key waits for it and looks again rather
    /// than asking for the same bars alongside it.
    /// Answers whether anything had to be fetched.
    async fn fill(&self, key: &Key, window: &Range<Timestamp>) -> Result<bool, String> {
        let key = loop {
            let waiter = {
                let mut inner = self.inner.borrow_mut();
                let key = inner.resolve(key);

                if inner.covered(&key, window) {
                    return Ok(false);
                }

                match inner.waiting.get_mut(&key) {
                    Some(waiting) => {
                        let (tx, rx) = oneshot::channel();
                        waiting.push(tx);

                        Some(rx)
                    }
                    None => {
                        inner.waiting.insert(key.clone(), Vec::new());

                        break key;
                    }
                }
            };

            // The other fetch may or may not have covered what this one wants,
            // so the loop asks again rather than assuming either way.
            let _ = waiter.expect("a waiter or the slot").await;
        };

        let result = self.fetch(&key, window).await;

        // Taken out before any of them are told, so that nothing they go on to
        // do finds the store still borrowed.
        let waiting = self
            .inner
            .borrow_mut()
            .waiting
            .remove(&key)
            .unwrap_or_default();

        for waiting in waiting {
            let _ = waiting.send(());
        }

        result.map(|()| true)
    }

    async fn fetch(&self, key: &Key, window: &Range<Timestamp>) -> Result<(), String> {
        let missing = {
            let inner = self.inner.borrow();
            let held = inner.series.get(key).map(|entry| &entry.coverage);

            plan(
                held.map(|c| c.missing(window))
                    .unwrap_or_else(|| vec![window.clone()]),
                key.interval,
            )
        };

        for span in missing {
            let fetched = self.source.fetch(&key.symbol, key.interval, span).await?;

            self.inner.borrow_mut().insert(key, fetched);
        }

        Ok(())
    }
}

impl<Snapshot> Inner<Snapshot> {
    /// The key a request for this one really lands on, once what the source
    /// has already substituted is taken into account.
    fn resolve(&self, key: &Key) -> Key {
        match self.substitutes.get(key) {
            Some(&interval) => Key {
                symbol: key.symbol.clone(),
                interval,
            },
            None => key.clone(),
        }
    }

    fn covered(&self, key: &Key, window: &Range<Timestamp>) -> bool {
        self.series
            .get(key)
            .is_some_and(|entry| entry.coverage.covers(window))
    }

    fn insert(&mut self, requested: &Key, fetched: Fetched<Snapshot>) {
        if fetched.interval != requested.interval {
            // Worth a line of its own: from here on, requests for one interval
            // quietly land on another, and a log without this reads as if the
            // cache were answering the wrong question.
            tracing::info!(
                target: TARGET,
                symbol = requested.symbol,
                asked = ?requested.interval,
                served = ?fetched.interval,
                "source substituted an interval",
            );

            self.substitutes.insert(requested.clone(), fetched.interval);
        }

        let entry = self
            .series
            .entry(Key {
                symbol: requested.symbol.clone(),
                interval: fetched.interval,
            })
            .or_default();

        entry.bars.merge(fetched.bars);
        entry.coverage.add(fetched.covered);

        if let Some(snapshot) = fetched.snapshot {
            self.snapshots.insert(requested.symbol.clone(), snapshot);
        }
    }
}

/// Which requests to make for a set of holes: the ones far enough apart to be
/// worth asking about separately, and otherwise one covering them all.
fn plan(missing: Vec<Range<Timestamp>>, interval: Interval) -> Vec<Range<Timestamp>> {
    let slack = GAP_SLACK_BARS * interval.seconds();
    let mut spans: Vec<Range<Timestamp>> = Vec::new();

    for span in missing {
        match spans.last_mut() {
            Some(last) if span.start.saturating_sub(last.end) <= slack => last.end = span.end,
            _ => spans.push(span),
        }
    }

    match (spans.first(), spans.last()) {
        // One request over the lot, not one range per timestamp in it.
        (Some(first), Some(last)) if spans.len() > MAX_REQUESTS => {
            std::iter::once(first.start..last.end).collect()
        }
        _ => spans,
    }
}

impl<S: Source> Clone for Store<S> {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
// These tests compare a list of spans against a literal list of ranges, which
// is exactly what it looks like. Clippy reads `[a..b]` as a botched attempt at
// a range of values instead.
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use std::cell::Cell;

    use super::*;

    /// What a fake source was asked for, in order.
    type Calls = Rc<RefCell<Vec<(Interval, Range<Timestamp>)>>>;

    /// A source that counts what it was asked for and answers with one bar per
    /// interval across the window.
    struct Fake {
        calls: Calls,
        serves: Cell<Option<Interval>>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                calls: Rc::new(RefCell::new(Vec::new())),
                serves: Cell::new(None),
            }
        }

        /// A source that will not serve the interval it is asked for.
        fn serving(interval: Interval) -> Self {
            let fake = Self::new();
            fake.serves.set(Some(interval));

            fake
        }
    }

    impl Source for Fake {
        type Snapshot = String;

        async fn fetch(
            &self,
            symbol: &str,
            interval: Interval,
            window: Range<Timestamp>,
        ) -> Result<Fetched<String>, String> {
            self.calls.borrow_mut().push((interval, window.clone()));

            let interval = self.serves.get().unwrap_or(interval);
            let mut bars = Bars::default();
            let mut at = interval.floor(window.start);

            while at < window.end {
                bars.push(at, at as f64, at as f64, 1.0);
                at += interval.seconds();
            }

            Ok(Fetched {
                interval,
                covered: window,
                bars,
                snapshot: Some(symbol.to_string()),
            })
        }
    }

    fn store() -> (Store<Fake>, Calls) {
        let source = Fake::new();
        let calls = source.calls.clone();

        (Store::new(source), calls)
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        futures::executor::block_on(future)
    }

    const MIN: u64 = 60;

    /// The case the whole design is for: two windows fetched, then the window
    /// spanning them both, which goes nowhere near the source.
    #[test]
    fn a_spanning_window_asks_for_nothing() {
        let (store, calls) = store();

        block_on(store.bars("SPY", Interval::Min1, 0..10 * MIN)).unwrap();
        block_on(store.bars("SPY", Interval::Min1, 10 * MIN..20 * MIN)).unwrap();

        assert_eq!(calls.borrow().len(), 2);

        let bars = block_on(store.bars("SPY", Interval::Min1, 0..20 * MIN)).unwrap();

        assert_eq!(calls.borrow().len(), 2);
        assert_eq!(bars.len(), 20);
    }

    #[test]
    fn asks_only_for_the_part_it_lacks() {
        let (store, calls) = store();

        block_on(store.bars("SPY", Interval::Min1, 0..10 * MIN)).unwrap();
        block_on(store.bars("SPY", Interval::Min1, 0..15 * MIN)).unwrap();

        assert_eq!(calls.borrow()[1], (Interval::Min1, 10 * MIN..15 * MIN));
    }

    #[test]
    fn a_narrower_window_is_free() {
        let (store, calls) = store();

        block_on(store.bars("SPY", Interval::Min1, 0..20 * MIN)).unwrap();
        let bars = block_on(store.bars("SPY", Interval::Min1, 5 * MIN..10 * MIN)).unwrap();

        assert_eq!(calls.borrow().len(), 1);
        assert_eq!(bars.len(), 5);
    }

    /// Intervals are held apart: bars of one size say nothing about another.
    #[test]
    fn intervals_do_not_answer_for_each_other() {
        let (store, calls) = store();

        block_on(store.bars("SPY", Interval::Min1, 0..20 * MIN)).unwrap();
        block_on(store.bars("SPY", Interval::Min5, 0..20 * MIN)).unwrap();

        assert_eq!(calls.borrow().len(), 2);
    }

    #[test]
    fn symbols_are_held_apart() {
        let (store, calls) = store();

        block_on(store.bars("SPY", Interval::Min1, 0..MIN)).unwrap();
        block_on(store.bars("QQQ", Interval::Min1, 0..MIN)).unwrap();

        assert_eq!(calls.borrow().len(), 2);
    }

    /// A source that will not serve what it was asked for is taken at its
    /// word, and not asked the same way twice.
    #[test]
    fn remembers_what_a_source_substitutes() {
        let source = Fake::serving(Interval::Hour1);
        let calls = source.calls.clone();
        let store = Store::new(source);

        block_on(store.bars("BTC", Interval::Min1, 0..3_600)).unwrap();
        let bars = block_on(store.bars("BTC", Interval::Min1, 0..3_600)).unwrap();

        assert_eq!(calls.borrow().len(), 1);
        assert_eq!(calls.borrow()[0].0, Interval::Min1);
        assert_eq!(bars.len(), 1);

        // And the bars are found under the interval that came back.
        assert_eq!(store.held("BTC", Interval::Hour1, &(0..3_600)).len(), 1);
    }

    #[test]
    fn keeps_the_snapshot_from_the_last_fetch() {
        let (store, _) = store();

        assert_eq!(store.snapshot("SPY"), None);

        block_on(store.bars("SPY", Interval::Min1, 0..MIN)).unwrap();

        assert_eq!(store.snapshot("SPY").as_deref(), Some("SPY"));
    }

    #[test]
    fn merges_holes_too_close_to_be_worth_a_request() {
        let plan = plan(vec![0..60, 120..180], Interval::Min1);

        assert_eq!(plan, [0..180]);
    }

    #[test]
    fn keeps_holes_far_apart_apart() {
        let plan = plan(vec![0..60, 6_000..6_060], Interval::Min1);

        assert_eq!(plan, [0..60, 6_000..6_060]);
    }

    /// Past a few holes, one request over the lot beats a request each.
    #[test]
    fn folds_many_holes_into_one_request() {
        let holes = (0..10).map(|i| i * 6_000..i * 6_000 + 60).collect();

        assert_eq!(plan(holes, Interval::Min1), [0..54_060]);
    }
}
