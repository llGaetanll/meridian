use api_common::ChartData;
use api_common::Price;
use api_common::Timestamp;
use serde::Deserialize;

use crate::YIndicators;
use crate::YMeta;

#[derive(Debug, Deserialize)]
pub struct YQuoteBlock {
    pub meta: YMeta,

    #[serde(default)]
    pub timestamp: Vec<Timestamp>,

    pub indicators: YIndicators,
}

/// Pure projections of a quote block onto the app's own types. Yahoo leaves
/// gaps in its series (halted minutes, holidays), so every one of these drops
/// the points it has no value for.
impl YQuoteBlock {
    pub fn close_series(&self) -> ChartData {
        self.series(|quote| &quote.close)
    }

    pub fn open_series(&self) -> ChartData {
        self.series(|quote| &quote.open)
    }

    /// Volume for exactly the points [`Self::close_series`] keeps, so the two
    /// stay index-aligned however many gaps Yahoo left. A bar with no reported
    /// volume counts as zero rather than shifting every point after it.
    pub fn volume_series(&self) -> ChartData {
        let Some(quote) = self.indicators.quote.first() else {
            return ChartData::default();
        };

        self.timestamp
            .iter()
            .zip(quote.close.iter())
            .zip(quote.volume.iter())
            .filter(|((_, close), _)| close.is_some())
            .map(|((&ts, _), volume)| (ts, volume.unwrap_or(0) as Price))
            .collect()
    }

    /// Last traded price: the live quote if Yahoo sent one, else the last close.
    pub fn last_price(&self) -> Option<Price> {
        self.meta
            .regular_market_price
            .or_else(|| self.close_series().last().map(|(_, px)| *px))
    }

    /// The session's opening price, i.e. the first point we have.
    pub fn first_open(&self) -> Option<Price> {
        self.open_series().first().map(|(_, px)| *px)
    }

    fn series<'a, F>(&'a self, pick: F) -> ChartData
    where
        F: Fn(&'a crate::YQuoteData) -> &'a Vec<Option<f64>>,
    {
        let Some(quote) = self.indicators.quote.first() else {
            return ChartData::default();
        };

        self.timestamp
            .iter()
            .zip(pick(quote).iter())
            .filter_map(|(&ts, px)| px.map(|px| (ts, px)))
            .collect()
    }
}
