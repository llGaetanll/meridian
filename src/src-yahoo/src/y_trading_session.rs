use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct YTradingSession {
    pub timezone: String,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub start: DateTime<Utc>,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub end: DateTime<Utc>,

    pub gmtoffset: i32,
}
