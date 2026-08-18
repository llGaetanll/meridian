//! Asking for an asset, and what comes back.
//!
//! This is the whole of what the pages see of the outside world. Behind it is
//! a cache of bars, so asking for a range twice, or for a range inside one
//! already looked at, costs nothing.

use api_cache::Store;
use api_common::ChartData;
use api_common::Price;
use api_common::Timestamp;
use chrono::Utc;
use tracing::Instrument;

use crate::io::ChartRange;
use crate::io::yahoo::YahooSource;

/// One asset's price history over some range, and the facts that go with it.
///
/// The series are as the source sent them. Thinning them to fit a chart is the
/// chart's business, and doing it here would leave nothing worth keeping.
#[derive(Debug, Clone)]
pub struct AssetQuote {
    /// As the source names it. The header reads the symbol the page was
    /// pointed at, so these two are carried but not yet shown.
    #[allow(dead_code)]
    pub symbol: String,
    #[allow(dead_code)]
    pub name: String,
    /// Classification line for the header, e.g. "ETF · USD · NYSEArca".
    pub details: String,
    pub price: Price,
    /// The first point of the series — the open for an intraday range.
    pub open: Option<Price>,
    pub previous_close: Option<Price>,
    pub prices: ChartData,
    pub volume: ChartData,
    /// Facts as the source states them, ready to list.
    pub key_stats: Vec<(String, String)>,
    /// Seconds to add to a UTC timestamp to read it in exchange local time.
    pub utc_offset: i64,
}

/// The market data the app runs on, cached across pages and ranges.
#[derive(Clone)]
pub struct Markets {
    store: Store<YahooSource>,
}

impl Markets {
    pub fn new() -> Self {
        Self {
            store: Store::new(YahooSource::new()),
        }
    }

    pub async fn quote(&self, symbol: &str, range: ChartRange) -> Result<AssetQuote, String> {
        // Everything the ask sets off — cache decisions, requests, responses —
        // is logged inside this span, so a line can always be read back to the
        // thing the user was looking at when it happened.
        let span = tracing::info_span!("quote", symbol, range = range.label());

        self.quote_inner(symbol, range).instrument(span).await
    }

    async fn quote_inner(&self, symbol: &str, range: ChartRange) -> Result<AssetQuote, String> {
        let window = range.window(now());
        let interval = range.interval();

        // What was asked for, in the terms it was asked in. Everything below
        // is in seconds and intervals; this is the last line that says which
        // button was pressed. The span already carries the symbol and range.
        tracing::info!(target: "app", "quote asked for");

        let bars = self.store.bars(symbol, interval, window.clone()).await?;

        let info = self
            .store
            .snapshot(symbol)
            .ok_or_else(|| format!("No data for {symbol}"))?;

        // A day's chart is the session, not the last 24 hours of clock: the
        // window is wider than that so that consecutive days can be merged,
        // and the tail of the previous one is trimmed off here. A window with
        // no session in it, over a weekend say, is left as it is.
        let bars = match range {
            ChartRange::Day => match &info.session {
                Some(session) if bars.window(session).len() > 1 => bars.window(session),
                _ => bars,
            },
            _ => bars,
        };

        Ok(AssetQuote {
            symbol: info.symbol,
            name: info.name,
            details: info.details,
            price: info.price.or(bars.last_close()).unwrap_or_default(),
            open: bars.first_open(),
            previous_close: info.previous_close,
            prices: bars.closes(),
            volume: bars.volumes(),
            key_stats: info.key_stats,
            utc_offset: info.utc_offset,
        })
    }
}

impl Default for Markets {
    fn default() -> Self {
        Self::new()
    }
}

fn now() -> Timestamp {
    Utc::now().timestamp().max(0) as Timestamp
}
