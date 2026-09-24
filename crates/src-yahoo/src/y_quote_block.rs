use api_common::AssetFacts;
use api_common::Bar;
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

    /// The series as bars, keeping exactly the points that have a close.
    ///
    /// A point with no close is a minute nothing traded and nothing to hold;
    /// the rest of a candle stays optional because Yahoo leaves holes in each
    /// field independently.
    pub fn bars(&self) -> Vec<Bar> {
        let Some(quote) = self.indicators.quote.first() else {
            return Vec::new();
        };

        self.timestamp
            .iter()
            .enumerate()
            .filter_map(|(i, &ts)| {
                let close = *quote.close.get(i)?;

                close.map(|close| Bar {
                    ts,
                    open: quote.open.get(i).copied().flatten(),
                    high: quote.high.get(i).copied().flatten(),
                    low: quote.low.get(i).copied().flatten(),
                    close,
                    volume: quote.volume.get(i).copied().flatten().unwrap_or(0),
                })
            })
            .collect()
    }

    /// What Yahoo says about the asset itself, off the chart's metadata.
    ///
    /// Facts Yahoo leaves out are left out rather than guessed at; what it
    /// states two ways (the previous close, the exchange's name) is taken in
    /// its preferred order.
    pub fn facts(&self) -> AssetFacts {
        let meta = &self.meta;

        let details = [
            meta.instrument_type.clone(),
            Some(meta.currency.clone()),
            meta.full_exchange_name
                .clone()
                .or_else(|| meta.exchange_name.clone()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");

        let session = meta
            .current_trading_period
            .as_ref()
            .and_then(|period| period.regular.as_ref())
            .map(|regular| {
                Timestamp::from_secs(regular.start.timestamp().max(0) as u64)
                    ..Timestamp::from_secs(regular.end.timestamp().max(0) as u64)
            });

        AssetFacts {
            name: meta.long_name.clone().or_else(|| meta.short_name.clone()),
            details: (!details.is_empty()).then_some(details),
            currency: Some(meta.currency.clone()),
            utc_offset: meta.gmtoffset.unwrap_or_default(),
            price: meta.regular_market_price,
            previous_close: meta.previous_close.or(meta.chart_previous_close),
            day_low: meta.regular_market_day_low,
            day_high: meta.regular_market_day_high,
            year_low: meta.fifty_two_week_low,
            year_high: meta.fifty_two_week_high,
            day_volume: meta.regular_market_volume.map(|volume| volume as u64),
            session,
        }
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
