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
    fn path(&self) -> String {
        match self {
            YRequestChart::Period {
                symbol,
                start,
                end,
                interval,
            } => {
                format!(
                    "v8/finance/chart/{}?period1={}&period2={}&interval={}",
                    symbol,
                    start.timestamp(),
                    end.timestamp(),
                    interval
                )
            }
            YRequestChart::Range {
                symbol,
                range,
                interval,
            } => {
                format!(
                    "v8/finance/chart/{}?range={}&interval={}",
                    symbol, range, interval
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn builds_a_range_path() {
        let req = YRequestChart::range("SPY", YRange::DayOne, YInterval::MinuteOne);

        assert_eq!(req.path(), "v8/finance/chart/SPY?range=1d&interval=1m");
    }

    #[test]
    fn builds_a_period_path() {
        let start = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let end = Utc.timestamp_opt(1_700_086_400, 0).unwrap();
        let req = YRequestChart::period("BTC-USD", start, end, YInterval::DayOne);

        assert_eq!(
            req.path(),
            "v8/finance/chart/BTC-USD?period1=1700000000&period2=1700086400&interval=1d"
        );
    }
}
