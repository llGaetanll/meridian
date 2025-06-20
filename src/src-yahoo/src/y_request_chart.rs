use chrono::DateTime;
use chrono::Utc;

use crate::YInterval;
use crate::YRange;
use crate::YRequest;

pub enum YRequestChart<'a> {
    Period {
        symbol: &'a str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval: YInterval,
    },
    Range {
        symbol: &'a str,
        range: YRange,
        interval: YInterval,
    },
}

impl<'a> YRequestChart<'a> {
    /// Create a new chart request using absolute time period
    pub fn period(
        symbol: &'a str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval: YInterval,
    ) -> Self {
        YRequestChart::Period {
            symbol,
            start,
            end,
            interval,
        }
    }

    /// Create a new chart request using relative range
    pub fn range(symbol: &'a str, range: YRange, interval: YInterval) -> Self {
        YRequestChart::Range {
            symbol,
            range,
            interval,
        }
    }
}

impl<'a> YRequest for YRequestChart<'a> {
    fn url(&self) -> reqwest::Url {
        let url = match self {
            YRequestChart::Period {
                symbol,
                start,
                end,
                interval,
            } => {
                format!(
                    "https://query1.finance.yahoo.com/v8/finance/chart/{}?period1={}&period2={}&interval={}",
                    symbol, start.timestamp(), end.timestamp(), interval
                )
            }
            YRequestChart::Range {
                symbol,
                range,
                interval,
            } => {
                format!(
                    "https://query1.finance.yahoo.com/v8/finance/chart/{}?range={}&interval={}",
                    symbol, range, interval
                )
            }
        };

        url.parse().unwrap() // won't fail
    }
}
