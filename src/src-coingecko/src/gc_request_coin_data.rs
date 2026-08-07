use chrono::DateTime;
use chrono::Utc;

use crate::CoinId;
use crate::Currency;
use crate::GcRequest;

// API Details
// -----------
//
//  You may leave the interval params as empty for automatic granularity:
//      1 day from current time = 5-minutely data
//      1 day from any time (except current time) = hourly data
//      2 - 90 days from any time = hourly data
//      above 90 days from any time = daily data (00:00 UTC)
//  Cache / Update Frequency:
//  Based on days range (all the API plans)
//      1 day = 30 seconds cache
//      2 -90 days = 30 minutes cache
//      90 days = 12 hours cache
//  The last completed UTC day (00:00) is available 35 minutes after midnight on the next UTC day (00:35). The cache will always expire at 00:40 UTC.
//  Access to historical data via the Public API (Demo plan) is restricted to the past 365 days only. To access the complete range of historical data, please subscribe to one of our paid plans to obtain a Pro-API key.

// See: http://docs.coingecko.com/v3.0.1/reference/coins-id-market-chart-range
pub struct GcRequestCoinData<'a> {
    id: CoinId<'a>,

    vs: Currency<'a>,

    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

impl<'a> GcRequestCoinData<'a> {
    pub fn vs_usd(id: CoinId<'a>, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            id,
            vs: "usd",
            start,
            end,
        }
    }
}

impl GcRequest for GcRequestCoinData<'_> {
    fn url(&self) -> reqwest::Url {
        let GcRequestCoinData { id, vs, start, end } = self;
        let (start, end) = (start.timestamp(), end.timestamp());

        let url = format!(
            "https://api.coingecko.com/api/v3/coins/{id}/market_chart/range?vs_currency={vs}&from={start}&to={end}"
        );

        url.parse().unwrap() // won't fail
    }
}
