//! Reads for the coverage page: whole-table views of what the bars hold.
//!
//! The app never asks these questions — it reads one series at a time. The
//! coverage tool asks about every series at once, so its queries live here,
//! apart, where the app cannot reach for them by accident.

use std::ops::Range;

use api_common::Timestamp;

use crate::Db;
use crate::PostgresError;
use crate::postgres_bars::BarInterval;
use crate::postgres_bars::BarSource;
use crate::postgres_ledger::seconds;
use crate::postgres_ledger::stamp;

/// One coverage span, with the symbol it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageSpan {
    pub symbol: String,
    pub span: Range<Timestamp>,
}

/// What one series holds since a moment: how many bars, and their edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesStats {
    pub symbol: String,
    pub bars: u64,
    pub first: Timestamp,
    pub last: Timestamp,
}

/// One point of a thinned curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosePoint {
    pub ts: Timestamp,
    pub close: f64,
}

/// Every coverage span a source holds at an interval, sorted by symbol and
/// then by time.
pub async fn load_coverage_spans(
    db: &Db,
    source: BarSource,
    interval: BarInterval,
) -> Result<Vec<CoverageSpan>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT symbol, span_start, span_end FROM bar_coverage
           WHERE source = $1 AND interval = $2
           ORDER BY symbol, span_start"#,
        source as BarSource,
        interval as BarInterval,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| CoverageSpan {
            symbol: row.symbol,
            span: seconds(row.span_start)..seconds(row.span_end),
        })
        .collect())
}

/// Bar counts and edges for every series a source holds at an interval,
/// counted from `since`. A series with no bars past `since` is absent.
pub async fn load_series_stats(
    db: &Db,
    source: BarSource,
    interval: BarInterval,
    since: Timestamp,
) -> Result<Vec<SeriesStats>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT symbol,
                  count(*) AS "bars!",
                  min(ts) AS "first!",
                  max(ts) AS "last!"
           FROM bars
           WHERE source = $1 AND interval = $2 AND ts >= $3
           GROUP BY symbol
           ORDER BY symbol"#,
        source as BarSource,
        interval as BarInterval,
        stamp(since),
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| SeriesStats {
            symbol: row.symbol,
            bars: row.bars.max(0) as u64,
            first: seconds(row.first),
            last: seconds(row.last),
        })
        .collect())
}

/// What a source last said an asset is called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetHead {
    pub symbol: String,
    pub name: Option<String>,
}

/// Every asset a source has spoken about, by symbol. A symbol with bars but
/// no asset row is simply absent.
pub async fn load_asset_heads(db: &Db, source: BarSource) -> Result<Vec<AssetHead>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT symbol, name FROM assets WHERE source = $1 ORDER BY symbol"#,
        source as BarSource,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AssetHead {
            symbol: row.symbol,
            name: row.name,
        })
        .collect())
}

/// How much money changes hands in one series on an ordinary day: volume
/// times close, averaged over the newest `days` daily bars. Dollars rather
/// than shares, because a $35 stock trades many more shares than an ETF at
/// $700 while mattering less.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesTurnover {
    pub symbol: String,
    pub turnover: f64,
}

/// The turnover of every series a source holds daily bars for.
pub async fn load_turnovers(
    db: &Db,
    source: BarSource,
    days: u32,
) -> Result<Vec<SeriesTurnover>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT symbol, avg(volume * close)::float8 AS "turnover!" FROM (
               SELECT symbol, volume, close,
                      row_number() OVER (PARTITION BY symbol ORDER BY ts DESC) AS rn
               FROM bars
               WHERE source = $1 AND interval = '1d'
           ) newest
           WHERE rn <= $2
           GROUP BY symbol"#,
        source as BarSource,
        i64::from(days.max(1)),
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| SeriesTurnover {
            symbol: row.symbol,
            turnover: row.turnover,
        })
        .collect())
}

/// One series' closes since a moment, thinned to at most about `target`
/// points, evenly by count. The first bar is always kept; the thinning is
/// for drawing, where two thousand points and two hundred are the same line.
pub async fn load_closes_thinned(
    db: &Db,
    source: BarSource,
    symbol: &str,
    interval: BarInterval,
    since: Timestamp,
    target: u32,
) -> Result<Vec<ClosePoint>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT ts, close FROM (
               SELECT ts, close,
                      row_number() OVER (ORDER BY ts) AS rn,
                      count(*) OVER () AS total
               FROM bars
               WHERE source = $1 AND symbol = $2 AND interval = $3 AND ts >= $4
           ) points
           WHERE (rn - 1) % GREATEST(total / $5, 1) = 0
           ORDER BY ts"#,
        source as BarSource,
        symbol,
        interval as BarInterval,
        stamp(since),
        i64::from(target.max(1)),
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ClosePoint {
            ts: seconds(row.ts),
            close: row.close,
        })
        .collect())
}
