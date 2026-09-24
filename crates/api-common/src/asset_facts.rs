use std::ops::Range;

use crate::Price;
use crate::Timestamp;

/// What a source says about the asset itself, as opposed to its price
/// history.
///
/// These arrive attached to every chart fetch, and they age in seconds where
/// a closed bar never ages at all — so they are stored apart from the bars
/// and replaced whole on every fetch. Everything is optional but the offset:
/// a source states what it states, and a fact it left out is left out here
/// rather than guessed at.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AssetFacts {
    pub name: Option<String>,

    /// Classification line for a header, e.g. "ETF · USD · NYSEArca".
    pub details: Option<String>,

    pub currency: Option<String>,

    /// Seconds to add to a UTC timestamp to read it in exchange local time.
    pub utc_offset: i64,

    /// The live price, when the market is open enough to have one.
    pub price: Option<Price>,

    pub previous_close: Option<Price>,

    pub day_low: Option<Price>,
    pub day_high: Option<Price>,

    pub year_low: Option<Price>,
    pub year_high: Option<Price>,

    /// Units traded so far in the regular session.
    pub day_volume: Option<u64>,

    /// The regular session in progress, or the last one to have run.
    pub session: Option<Range<Timestamp>>,
}
