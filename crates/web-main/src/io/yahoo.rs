//! Yahoo Finance as something the cache can pull bars from.
//!
//! Nothing here is reactive: these are plain async functions over plain data,
//! so they can be called from any front-end, a test, or a background job.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use api_cache::Bars;
use api_cache::Fetched;
use api_cache::Interval;
use api_cache::Source;
use api_common::Price;
use api_common::Timestamp;
use api_common::format::fmt_compact;
use api_common::format::fmt_money;
use chrono::DateTime;
use chrono::Utc;
use src_yahoo::YInterval;
use src_yahoo::YQuoteBlock;
use src_yahoo::YRequestChart;
use src_yahoo::YResponseChart;
use src_yahoo::YahooClient;

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

const DAY: u64 = 86_400;

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

    /// The stretch of time the range covers, ending now.
    ///
    /// Windows are absolute rather than relative — "the last month" is a
    /// different month every day, and two of them cannot be merged, while
    /// two spans of seconds always can.
    pub fn window(&self, now: Timestamp) -> Range<Timestamp> {
        // It ends at the last bar to have closed rather than at this instant.
        // The bar still forming is revised until it closes, so asking for it
        // is asking for something that cannot be settled — and a window that
        // can never be settled is a window fetched again every time it is
        // looked at, which is the one thing worth avoiding here. What it costs
        // is a chart that stops a bar short of now: a minute on a day, a week
        // on thirty years.
        let end = self.interval().last_closed(now);

        let back = match self {
            Self::Day => DAY,
            Self::Week => 7 * DAY,
            Self::Month => 30 * DAY,
            Self::Year => 365 * DAY,
            Self::FiveYears => 5 * 365 * DAY,
            // Yahoo clamps to the first trade it has, whatever we ask for.
            Self::Max => return Timestamp::EPOCH..end,
        };

        now.saturating_sub_secs(back)..end
    }

    /// The finest interval that covers the range without asking for points so
    /// far past what a chart can show that we are only paying for bytes.
    pub fn interval(&self) -> Interval {
        match self {
            Self::Day => Interval::Min1,
            Self::Week => Interval::Min2,
            Self::Month => Interval::Min5,
            Self::Year => Interval::Hour1,
            Self::FiveYears => Interval::Day1,
            Self::Max => Interval::Week1,
        }
    }
}

/// What Yahoo says about the asset itself, as opposed to its price history.
///
/// This ages in seconds where a closed bar never ages at all, which is why the
/// cache keeps it apart from the bars and replaces it whole on every fetch.
#[derive(Debug, Clone)]
pub struct AssetInfo {
    pub symbol: String,
    pub name: String,
    /// Classification line for the header, e.g. "ETF · USD · NYSEArca".
    pub details: String,
    /// The live price, when the market is open enough to have one.
    pub price: Option<Price>,
    pub previous_close: Option<Price>,
    /// Facts as the source states them, ready to list.
    pub key_stats: Vec<(String, String)>,
    /// Seconds to add to a UTC timestamp to read it in exchange local time.
    pub utc_offset: i64,
    /// The regular session in progress, or the last one to have run.
    pub session: Option<Range<Timestamp>>,
}

pub struct YahooSource {
    /// Built on first use and kept: the browser build has to read its own
    /// origin to know where to send requests, which is not something a
    /// constructor should be able to fail at.
    client: RefCell<Option<Rc<YahooClient>>>,
}

impl YahooSource {
    pub fn new() -> Self {
        Self {
            client: RefCell::new(None),
        }
    }

    /// Yahoo is reachable directly from native builds. In the browser it is
    /// not — it sends no CORS headers — so requests go to this app's own
    /// origin, which forwards them (see the dev server proxy in `Trunk.toml`).
    fn client(&self) -> Result<Rc<YahooClient>, String> {
        if let Some(client) = self.client.borrow().as_ref() {
            return Ok(client.clone());
        }

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

        let client = Rc::new(client.map_err(|e| e.to_string())?);
        *self.client.borrow_mut() = Some(client.clone());

        Ok(client)
    }
}

impl Source for YahooSource {
    type Snapshot = AssetInfo;

    async fn fetch(
        &self,
        symbol: &str,
        interval: Interval,
        window: Range<Timestamp>,
    ) -> Result<Fetched<AssetInfo>, String> {
        let now = Timestamp::from_secs(Utc::now().timestamp().max(0) as u64);

        // Bars settle when they close and are revised until they do, so the
        // one still forming is left outside coverage and fetched again next
        // time. Everything behind it is fetched once and never again.
        let covered = window.start..window.end.min(interval.last_closed(now));

        let asked = servable(&window, interval, now);

        // Nothing left to ask for: Yahoo will not serve bars this fine this
        // far back, and never will again. Recording the window as covered
        // says so, rather than asking every time it is looked at.
        let Some(asked) = asked else {
            return Ok(Fetched {
                interval,
                covered,
                bars: Bars::default(),
                snapshot: None,
            });
        };

        let request = YRequestChart::period(
            symbol,
            stamp(asked.start),
            stamp(asked.end),
            yahoo_interval(interval),
        );

        let response: YResponseChart = self
            .client()?
            .send_request(&request)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(error) = response.chart.error {
            return Err(error.to_string());
        }

        let block = response
            .chart
            .result
            .unwrap_or_default()
            .into_iter()
            .next()
            .ok_or_else(|| format!("No data for {symbol}"))?;

        Ok(Fetched {
            // Yahoo states the granularity it served, which is not always the
            // one asked for.
            interval: served_interval(&block).unwrap_or(interval),
            covered,
            bars: bars(&block),
            snapshot: Some(info(&block)),
        })
    }
}

/// The part of `window` Yahoo will still serve at this interval, if any.
///
/// Intraday bars fall off the back of the API as they age, at a different age
/// per interval. Asking for them anyway is answered with an error rather than
/// with what is left, so the ask is trimmed to what is left.
fn servable(
    window: &Range<Timestamp>,
    interval: Interval,
    now: Timestamp,
) -> Option<Range<Timestamp>> {
    let max_age = match interval {
        Interval::Min1 => 7 * DAY,
        Interval::Hour1 => 730 * DAY,
        Interval::Day1 | Interval::Week1 => return Some(window.clone()),
        _ => 60 * DAY,
    };

    // A bar of margin: the cut-off moves while the request is in flight.
    let oldest =
        now.saturating_sub_secs(max_age) + std::time::Duration::from_secs(interval.seconds());
    let asked = window.start.max(oldest)..window.end;

    (asked.start < asked.end).then_some(asked)
}

fn stamp(ts: Timestamp) -> DateTime<Utc> {
    DateTime::from_timestamp(ts.as_secs() as i64, 0).unwrap_or_default()
}

fn yahoo_interval(interval: Interval) -> YInterval {
    match interval {
        Interval::Min1 => YInterval::MinuteOne,
        Interval::Min2 => YInterval::MinuteTwo,
        Interval::Min5 => YInterval::MinuteFive,
        Interval::Min15 => YInterval::MinuteFifteen,
        Interval::Min30 => YInterval::MinuteThirty,
        Interval::Hour1 => YInterval::HourOne,
        Interval::Day1 => YInterval::DayOne,
        Interval::Week1 => YInterval::WeekOne,
    }
}

/// What Yahoo says it served, read back off the response.
fn served_interval(block: &YQuoteBlock) -> Option<Interval> {
    let granularity = block.meta.data_granularity.as_deref()?;

    Interval::ALL
        .into_iter()
        .find(|&interval| yahoo_interval(interval).to_string() == granularity)
}

/// The bars of a quote block, dropping the ones Yahoo has no close for —
/// halted minutes and the like. A bar with no reported volume counts as zero
/// rather than shifting every bar after it.
fn bars(block: &YQuoteBlock) -> Bars {
    let mut bars = Bars::default();

    let Some(quote) = block.indicators.quote.first() else {
        return bars;
    };

    for (i, &ts) in block.timestamp.iter().enumerate() {
        let Some(Some(close)) = quote.close.get(i) else {
            continue;
        };

        let open = quote.open.get(i).copied().flatten().unwrap_or(*close);
        let volume = quote.volume.get(i).copied().flatten().unwrap_or(0);

        bars.push(ts, open, *close, volume as Price);
    }

    bars
}

fn info(block: &YQuoteBlock) -> AssetInfo {
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

    let session = meta
        .current_trading_period
        .as_ref()
        .and_then(|period| period.regular.as_ref())
        .map(|regular| {
            Timestamp::from_secs(regular.start.timestamp().max(0) as u64)
                ..Timestamp::from_secs(regular.end.timestamp().max(0) as u64)
        });

    AssetInfo {
        symbol: meta.symbol.clone(),
        name,
        details,
        price: meta.regular_market_price,
        previous_close: meta.previous_close.or(meta.chart_previous_close),
        key_stats: key_stats(block, first_open(block)),
        utc_offset: meta.gmtoffset.unwrap_or_default(),
        session,
    }
}

/// The price the first bar of the block opened at.
fn first_open(block: &YQuoteBlock) -> Option<Price> {
    let quote = block.indicators.quote.first()?;

    quote
        .open
        .iter()
        .zip(quote.close.iter())
        .find_map(|(open, close)| open.or(*close))
}

/// The facts Yahoo returns alongside a chart. Anything it leaves out is left
/// out here too, rather than guessed at.
fn key_stats(block: &YQuoteBlock, open: Option<Price>) -> Vec<(String, String)> {
    let meta = &block.meta;
    let mut stats = Vec::new();

    if let Some(open) = open {
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

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Timestamp = Timestamp::from_secs(1_786_000_000);

    /// `days_ago(7)` — the tests below are all written relative to `NOW`.
    fn days_ago(days: u64) -> Timestamp {
        NOW.saturating_sub_secs(days * DAY)
    }

    /// A window ends at the last bar to have *closed*, not at this instant —
    /// see `window`. `NOW` is not minute-aligned, so the end is a little
    /// before it, and asserting on `NOW` itself would be asserting the bug
    /// that comment exists to prevent.
    #[test]
    fn windows_end_at_the_last_closed_bar_and_reach_back() {
        let day = ChartRange::Day.window(NOW);

        assert_eq!(day.start, days_ago(1));
        assert_eq!(day.end, ChartRange::Day.interval().last_closed(NOW));
        assert!(day.end <= NOW);

        assert_eq!(ChartRange::Max.window(NOW).start, Timestamp::EPOCH);
    }

    /// A window reaching past what Yahoo will serve is trimmed rather than
    /// refused outright.
    #[test]
    fn trims_an_ask_to_what_is_still_served() {
        let asked = servable(&(Timestamp::EPOCH..NOW), Interval::Min1, NOW).unwrap();

        assert!(asked.start >= days_ago(7));
        assert_eq!(asked.end, NOW);
    }

    #[test]
    fn asks_for_nothing_once_bars_are_too_old_to_serve() {
        let window = days_ago(90)..days_ago(80);

        assert_eq!(servable(&window, Interval::Min5, NOW), None);
    }

    #[test]
    fn daily_bars_are_served_however_far_back() {
        assert_eq!(
            servable(&(Timestamp::EPOCH..NOW), Interval::Day1, NOW),
            Some(Timestamp::EPOCH..NOW)
        );
    }

    /// Every interval the cache knows has to name itself the way Yahoo does,
    /// or reading the granularity back off a response would find nothing.
    #[test]
    fn every_interval_maps_onto_a_yahoo_one() {
        for interval in Interval::ALL {
            let granularity = yahoo_interval(interval).to_string();

            assert_eq!(
                Interval::ALL
                    .into_iter()
                    .find(|&i| yahoo_interval(i).to_string() == granularity),
                Some(interval)
            );
        }
    }
}
