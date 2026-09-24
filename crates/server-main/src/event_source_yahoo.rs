//! Yahoo answered, or failed to.

use api_common::Bar;
use src_postgres::BarInterval;

use crate::YahooFault;

#[derive(Debug)]
pub enum EventSourceYahoo {
    /// Bars arrived, at the interval Yahoo actually served.
    ///
    /// The bars are points, not knowledge: which stretch of the window this
    /// settles is the machine's decision, because the live edge is still
    /// forming and only the machine holds the clock. Empty is a window with
    /// no trading in it, which is an answer like any other.
    ChartFetched {
        symbol: String,
        interval: BarInterval,
        bars: Vec<Bar>,
    },

    /// The fetch served nothing, and in which of the three ways.
    ChartFailed { symbol: String, fault: YahooFault },
}
