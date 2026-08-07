use std::ops::Range;

use api_common::Timestamp;

use crate::Bars;
use crate::Interval;

/// Somewhere bars come from.
///
/// A source is asked for an interval and a window, and answers with what it
/// actually served. The two need not match: granularity rules differ from one
/// API to the next and are rarely stated in full — CoinGecko picks the
/// interval from how wide the window is, Yahoo refuses intraday bars past a
/// certain age. Reporting rather than promising is what keeps those rules out
/// of the cache: it files what came back under what it is, and remembers the
/// substitution rather than being surprised by it twice.
pub trait Source {
    /// Whatever the source knows about the thing itself, as opposed to its
    /// price history — a name, a last traded price, the exchange's offset from
    /// UTC. It comes back attached to a fetch because that is how these APIs
    /// hand it over, but it ages in seconds where bars never age at all, so
    /// the cache keeps it apart from them.
    type Snapshot: Clone;

    /// Bars for `symbol` over `window`, at `interval` or as near as the source
    /// is willing to go.
    fn fetch(
        &self,
        symbol: &str,
        interval: Interval,
        window: Range<Timestamp>,
    ) -> impl Future<Output = Result<Fetched<Self::Snapshot>, String>>;
}

/// What a source served, in its own terms.
pub struct Fetched<Snapshot> {
    /// The interval of the bars below, which may not be the one asked for.
    pub interval: Interval,

    /// The stretch of time this settles, which the cache takes at its word.
    ///
    /// It is a claim about knowledge, not about points: a source that answers
    /// a week with three bars is saying the other days had no trading, and
    /// that is worth storing. Two things belong outside it — the bar still
    /// forming at the live edge, which is not a fact yet, and any part of the
    /// window the source would not serve.
    pub covered: Range<Timestamp>,

    pub bars: Bars,

    pub snapshot: Option<Snapshot>,
}
