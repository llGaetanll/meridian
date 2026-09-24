use crate::Price;
use crate::Timestamp;

/// One bar of a price series: what a symbol traded at over one interval.
///
/// `ts` is the start of the interval the bar stands for. The close is the one
/// value every source reports and every chart reads, so it is required where
/// the rest of the candle is not — Yahoo leaves holes in all four, and a bar
/// with a close is still worth holding when its open is missing.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub ts: Timestamp,
    pub open: Option<Price>,
    pub high: Option<Price>,
    pub low: Option<Price>,
    pub close: Price,

    /// Units traded over the interval. A source that reports none says zero,
    /// which keeps volume aligned with the bars rather than shifting them.
    pub volume: u64,
}
