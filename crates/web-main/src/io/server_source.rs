//! Meridian's own server as something the cache can pull bars from.
//!
//! The browser never talks to a market data source itself: it asks our
//! server, which holds everything anybody has ever asked for and fetches the
//! rest once for everyone. What sits in front of this is the same in-memory
//! cache that used to front Yahoo, so a range looked at twice in one visit
//! still costs no request at all.

use std::ops::Range;

use api_cache::Bars;
use api_cache::Fetched;
use api_cache::Interval;
use api_cache::Source;
use api_common::Price;
use api_common::Timestamp;
use api_common::format::fmt_compact;
use api_common::format::fmt_money;
use chrono::Utc;
use server_wire::WireAsset;
use server_wire::WireBars;

use crate::io::Meridian;
use crate::io::session;

/// What the source says about the asset itself, as opposed to its price
/// history.
///
/// This ages in seconds where a closed bar never ages at all, which is why
/// the cache keeps it apart from the bars and replaces it whole on every
/// fetch.
#[derive(Debug, Clone)]
pub struct AssetInfo {
    pub symbol: String,
    pub name: String,
    /// Classification line for the header, e.g. "ETF · USD · NYSEArca".
    pub details: String,
    /// The live price, as of the server's last trip to the source — which is
    /// not always recent. The chart's own number comes from the bars.
    pub price: Option<Price>,
    pub previous_close: Option<Price>,
    /// Facts as the source states them, ready to list.
    pub key_stats: Vec<(String, String)>,
    /// Seconds to add to a UTC timestamp to read it in exchange local time.
    pub utc_offset: i64,
    /// The regular session in progress, or the last one to have run.
    pub session: Option<Range<Timestamp>>,
}

pub struct ServerSource {
    meridian: Meridian,
}

impl ServerSource {
    pub fn new() -> Self {
        Self {
            meridian: Meridian::new(),
        }
    }
}

impl Source for ServerSource {
    type Snapshot = AssetInfo;

    async fn fetch(
        &self,
        symbol: &str,
        interval: Interval,
        window: Range<Timestamp>,
    ) -> Result<Fetched<AssetInfo>, String> {
        // Read at the ask rather than held, so signing out is not answered
        // with a token that has been forgotten.
        let token = session::load().ok_or("Not signed in")?;

        let answer: WireBars = self
            .meridian
            .bars(
                &token,
                symbol,
                word(interval),
                window.start.as_secs(),
                window.end.as_secs(),
            )
            .await
            .map_err(|e| format!("{e:?}"))?;

        let now = Timestamp::from_secs(Utc::now().timestamp().max(0) as u64);

        // The server settles everything it can — what a source refuses to
        // serve, it records as known — so the window is knowledge up to the
        // bar still forming, which is fetched again next time.
        let covered = window.start..window.end.min(interval.last_closed(now));

        let mut bars = Bars::default();

        for bar in &answer.bars {
            bars.push(
                Timestamp::from_secs(bar.ts),
                bar.open.unwrap_or(bar.close),
                bar.close,
                bar.volume as Price,
            );
        }

        Ok(Fetched {
            // The server names the interval it served, which is not always
            // the one asked for.
            interval: served(&answer.interval).unwrap_or(interval),
            covered,
            bars,
            snapshot: Some(info(symbol, answer.asset)),
        })
    }
}

/// The wire's word for each interval the cache knows, `1m` through `1wk` —
/// the same words `src_postgres::BarInterval` spells on the other side.
fn word(interval: Interval) -> &'static str {
    match interval {
        Interval::Min1 => "1m",
        Interval::Min2 => "2m",
        Interval::Min5 => "5m",
        Interval::Min15 => "15m",
        Interval::Min30 => "30m",
        Interval::Hour1 => "1h",
        Interval::Day1 => "1d",
        Interval::Week1 => "1wk",
    }
}

/// The interval a wire word names, `None` for a word this build does not
/// know — the ask is believed instead.
fn served(word_served: &str) -> Option<Interval> {
    Interval::ALL
        .into_iter()
        .find(|&interval| word(interval) == word_served)
}

/// The asset's facts, off the wire. A symbol the server has no row for yet
/// is dressed from its bars alone: the symbol for a name, and nothing said
/// that nobody said.
fn info(symbol: &str, asset: Option<WireAsset>) -> AssetInfo {
    let asset = asset.unwrap_or_default();

    AssetInfo {
        symbol: symbol.to_string(),
        name: asset.name.clone().unwrap_or_else(|| symbol.to_string()),
        details: asset.details.clone().unwrap_or_default(),
        price: asset.price,
        previous_close: asset.previous_close,
        key_stats: key_stats(&asset),
        utc_offset: asset.utc_offset,
        session: match (asset.session_start, asset.session_end) {
            (Some(start), Some(end)) => {
                Some(Timestamp::from_secs(start)..Timestamp::from_secs(end))
            }
            _ => None,
        },
    }
}

/// The facts worth listing, in the order the rail lists them. Anything the
/// source left out is left out here too, rather than guessed at.
fn key_stats(asset: &WireAsset) -> Vec<(String, String)> {
    let mut stats = Vec::new();

    if let Some(close) = asset.previous_close {
        stats.push(("Prev close", fmt_money(close)));
    }

    if let (Some(low), Some(high)) = (asset.day_low, asset.day_high) {
        stats.push((
            "Day range",
            format!("{} – {}", fmt_money(low), fmt_money(high)),
        ));
    }

    if let (Some(low), Some(high)) = (asset.year_low, asset.year_high) {
        stats.push((
            "52w range",
            format!("{} – {}", fmt_money(low), fmt_money(high)),
        ));
    }

    if let Some(volume) = asset.day_volume {
        stats.push(("Volume", fmt_compact(volume as f64)));
    }

    if let Some(currency) = &asset.currency {
        stats.push(("Currency", currency.clone()));
    }

    stats
        .into_iter()
        .map(|(label, value)| (label.to_string(), value))
        .collect()
}
