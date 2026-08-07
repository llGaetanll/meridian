use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;

use crate::YTradingPeriod;
use crate::YTradingSession;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YMeta {
    pub currency: String,

    pub symbol: String,

    pub exchange_name: Option<String>,

    pub full_exchange_name: Option<String>,

    pub long_name: Option<String>,

    pub short_name: Option<String>,

    pub instrument_type: Option<String>,

    #[serde(default, with = "chrono::serde::ts_seconds_option")]
    pub first_trade_date: Option<DateTime<Utc>>,

    #[serde(default, with = "chrono::serde::ts_seconds_option")]
    pub regular_market_time: Option<DateTime<Utc>>,

    pub regular_market_price: Option<f64>,

    pub chart_previous_close: Option<f64>,

    pub previous_close: Option<f64>,

    pub regular_market_day_high: Option<f64>,

    pub regular_market_day_low: Option<f64>,

    pub regular_market_volume: Option<f64>,

    pub fifty_two_week_high: Option<f64>,

    pub fifty_two_week_low: Option<f64>,

    /// Seconds to add to a UTC timestamp to get the exchange's local time.
    pub gmtoffset: Option<i64>,

    pub scale: Option<u32>,

    pub price_hint: Option<u32>,

    pub current_trading_period: Option<YTradingPeriod>,

    pub trading_periods: Option<Vec<Vec<YTradingSession>>>,

    pub data_granularity: Option<String>,

    pub range: Option<String>,

    pub valid_ranges: Option<Vec<String>>,
}
