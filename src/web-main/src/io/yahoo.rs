//! Fetching prices from Yahoo Finance and mapping them onto the app's types.
//!
//! Nothing here is reactive: these are plain async functions over plain data,
//! so they can be called from any front-end, a test, or a background job.

use api_common::ChartData;
use api_common::Price;
use api_common::format::fmt_compact;
use api_common::format::fmt_money;
use src_yahoo::YInterval;
use src_yahoo::YQuoteBlock;
use src_yahoo::YRange;
use src_yahoo::YRequestChart;
use src_yahoo::YResponseChart;
use src_yahoo::YahooClient;

/// How many points a chart is drawn from, whatever stretch of time it covers.
///
/// Roughly one per pixel of chart width on a desktop screen: past that the
/// extra points fall inside the same pixel and only cost bytes. Holding it
/// steady across ranges is what makes a month look as detailed as a day.
pub const CHART_RESOLUTION: usize = 900;

/// How much history to ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartRange {
    Day,
    Week,
    Month,
    Year,
    FiveYears,
    Max,
}

impl ChartRange {
    pub const ALL: [Self; 6] = [
        Self::Day,
        Self::Week,
        Self::Month,
        Self::Year,
        Self::FiveYears,
        Self::Max,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Day => "1D",
            Self::Week => "1W",
            Self::Month => "1M",
            Self::Year => "1Y",
            Self::FiveYears => "5Y",
            Self::Max => "All",
        }
    }

    fn range(&self) -> YRange {
        match self {
            Self::Day => YRange::DayOne,
            Self::Week => YRange::DayFive,
            Self::Month => YRange::MonthOne,
            Self::Year => YRange::YearOne,
            Self::FiveYears => YRange::YearFive,
            Self::Max => YRange::Max,
        }
    }

    /// The finest interval that covers the range without overshooting
    /// [`CHART_RESOLUTION`] so far that we are paying for points we then throw
    /// away. Anything still over the target gets resampled after it arrives.
    ///
    /// Two ceilings are not ours to raise: Yahoo stops serving one-minute data
    /// past 7 days and other intraday intervals past 60, and a single session
    /// only holds ~390 minutes, so 1D is as detailed as the market itself.
    fn interval(&self) -> YInterval {
        match self {
            Self::Day => YInterval::MinuteOne,
            Self::Week => YInterval::MinuteTwo,
            Self::Month => YInterval::MinuteFive,
            Self::Year => YInterval::HourOne,
            Self::FiveYears => YInterval::DayOne,
            // Yahoo hands back ~400 points for the full history whatever we ask.
            Self::Max => YInterval::WeekOne,
        }
    }
}

/// One asset's price history and the facts that go with it.
#[derive(Debug, Clone)]
pub struct AssetQuote {
    pub symbol: String,
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

pub async fn fetch_quote(symbol: &str, range: ChartRange) -> Result<AssetQuote, String> {
    let client = client()?;

    let request = YRequestChart::range(symbol, range.range(), range.interval());

    let response: YResponseChart = client
        .send_request(&request)
        .await
        .map_err(|e| e.to_string())?;

    let block = response
        .chart
        .result
        .into_iter()
        .next()
        .ok_or_else(|| format!("No data for {symbol}"))?;

    Ok(quote_from(block))
}

/// Yahoo is reachable directly from native builds. In the browser it is not —
/// it sends no CORS headers — so requests go to this app's own origin, which
/// forwards them (see the dev server proxy in `Trunk.toml`).
fn client() -> Result<YahooClient, String> {
    #[cfg(target_arch = "wasm32")]
    let client = {
        let origin = web_sys::window()
            .ok_or("No window")?
            .location()
            .origin()
            .map_err(|_| "No origin")?;

        YahooClient::with_base_url(&format!("{origin}/"))
    };

    #[cfg(not(target_arch = "wasm32"))]
    let client = YahooClient::new();

    client.map_err(|e| e.to_string())
}

fn quote_from(block: YQuoteBlock) -> AssetQuote {
    let meta = &block.meta;

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

    let name = meta
        .long_name
        .clone()
        .or_else(|| meta.short_name.clone())
        .unwrap_or_else(|| meta.symbol.clone());

    AssetQuote {
        symbol: meta.symbol.clone(),
        name,
        details,
        price: block.last_price().unwrap_or_default(),
        open: block.first_open(),
        previous_close: meta.previous_close.or(meta.chart_previous_close),
        key_stats: key_stats(&block),
        utc_offset: meta.gmtoffset.unwrap_or_default(),
        prices: block.close_series().resample_last(CHART_RESOLUTION),
        volume: block.volume_series().resample_sum(CHART_RESOLUTION),
    }
}

/// The facts Yahoo returns alongside a chart. Anything it leaves out is left
/// out here too, rather than guessed at.
fn key_stats(block: &YQuoteBlock) -> Vec<(String, String)> {
    let meta = &block.meta;
    let mut stats = Vec::new();

    if let Some(open) = block.first_open() {
        stats.push(("Open", fmt_money(open)));
    }

    if let Some(close) = meta.previous_close.or(meta.chart_previous_close) {
        stats.push(("Prev close", fmt_money(close)));
    }

    if let (Some(low), Some(high)) = (meta.regular_market_day_low, meta.regular_market_day_high) {
        stats.push((
            "Day range",
            format!("{} – {}", fmt_money(low), fmt_money(high)),
        ));
    }

    if let (Some(low), Some(high)) = (meta.fifty_two_week_low, meta.fifty_two_week_high) {
        stats.push((
            "52w range",
            format!("{} – {}", fmt_money(low), fmt_money(high)),
        ));
    }

    if let Some(volume) = meta.regular_market_volume {
        stats.push(("Volume", fmt_compact(volume)));
    }

    stats.push(("Currency", meta.currency.clone()));

    stats
        .into_iter()
        .map(|(label, value)| (label.to_string(), value))
        .collect()
}
