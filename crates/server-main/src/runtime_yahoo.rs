//! Yahoo Finance, as the server's actions reach it.
//!
//! Plain async functions over plain data, called by the runtime and by
//! nothing that decides anything. Yahoo asks for no credentials; what it
//! rations is requests, and how fast to send them is the machine's decision,
//! not made here.
//!
//! One call each, on the one [`client`] the process holds.

use api_common::Bar;
use api_common::Timestamp;
use chrono::DateTime;
use chrono::Utc;
use src_postgres::BarInterval;
use src_yahoo::YError;
use src_yahoo::YInterval;
use src_yahoo::YRequestChart;
use src_yahoo::YResponseChart;
use src_yahoo::YahooClient;

use crate::YahooFault;

/// The one client the process holds.
///
/// Built once at startup rather than per request, so the connections it pools
/// are reused across every fetch.
pub fn client() -> Result<YahooClient, YError> {
    YahooClient::new()
}

/// Bars for `symbol` over `start..end`, at `interval`.
///
/// What comes back is points, as Yahoo sent them: gaps and all, nothing
/// dropped but the values Yahoo left null. Empty is a window Yahoo answered
/// with no trading in it, which is not a fault.
pub async fn fetch_chart(
    client: &YahooClient,
    symbol: &str,
    interval: BarInterval,
    start: Timestamp,
    end: Timestamp,
) -> Result<Vec<Bar>, YahooFault> {
    let (Some(start), Some(end)) = (moment(start), moment(end)) else {
        return Err(YahooFault::Failed {
            message: format!("the window {}..{} is not a time", start, end),
        });
    };

    let request = YRequestChart::period(symbol, start, end, y_interval(interval));

    let response: YResponseChart = match client.send_request(&request).await {
        Ok(response) => response,
        Err(YError::TooManyRequests) => return Err(YahooFault::RateLimited),
        Err(error) => {
            return Err(YahooFault::Failed {
                message: error.to_string(),
            });
        }
    };

    if let Some(error) = response.chart.error {
        return Err(YahooFault::Refused {
            code: error.code,
            description: error.description,
        });
    }

    let blocks = response.chart.result.unwrap_or_default();

    Ok(blocks.first().map(|block| block.bars()).unwrap_or_default())
}

/// Yahoo's name for each interval the store knows.
fn y_interval(interval: BarInterval) -> YInterval {
    match interval {
        BarInterval::Min1 => YInterval::MinuteOne,
        BarInterval::Min2 => YInterval::MinuteTwo,
        BarInterval::Min5 => YInterval::MinuteFive,
        BarInterval::Min15 => YInterval::MinuteFifteen,
        BarInterval::Min30 => YInterval::MinuteThirty,
        BarInterval::Hour1 => YInterval::HourOne,
        BarInterval::Day1 => YInterval::DayOne,
        BarInterval::Week1 => YInterval::WeekOne,
    }
}

/// A timestamp as the request wants it. `None` falls outside chrono, which no
/// timestamp the machine hands over should.
fn moment(ts: Timestamp) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(ts.as_secs() as i64, 0)
}
