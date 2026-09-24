//! Something for Yahoo to do.

use api_common::Timestamp;
use src_postgres::BarInterval;

#[derive(Debug)]
pub enum ActionSourceYahoo {
    /// Bars for one symbol over one window, at one interval.
    ///
    /// The window is half-open, `start..end`. The interval is the one asked
    /// for; what Yahoo actually serves comes back on the event, because the
    /// two need not match — Yahoo refuses intraday bars past a certain age.
    FetchChart {
        symbol: String,
        interval: BarInterval,
        start: Timestamp,
        end: Timestamp,
    },
}
