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

        /// What Yahoo said about the asset itself, riding along because it
        /// arrives on every chart response and ages in seconds — the machine
        /// files it beside the bars, where the store replaces it whole.
        facts: Option<api_common::AssetFacts>,
    },

    /// The fetch served nothing, and in which of the three ways.
    ChartFailed { symbol: String, fault: YahooFault },
}
