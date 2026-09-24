//! Bars and the coverage that says which windows of them are settled.
//!
//! A window already fetched is answered from here without asking anyone, so
//! coverage is written in the same transaction as the bars it vouches for —
//! a claim standing without its points, or points nobody counts as known,
//! are both worse than a failed write.

use std::ops::Range;

use api_common::Bar;
use api_common::Timestamp;

use crate::Db;
use crate::PostgresError;
use crate::postgres_ledger::seconds;
use crate::postgres_ledger::stamp;

/// Where a series came from.
///
/// Mirrors the `bar_source` enum in the database. Symbols are the source's
/// own namespace — 'AAPL' is Yahoo's word and 'bitcoin' is CoinGecko's — so
/// a series is keyed by its source and two sources never mix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "bar_source", rename_all = "lowercase")]
pub enum BarSource {
    Yahoo,
    Coingecko,
}

/// How much time one bar stands for.
///
/// Mirrors the `bar_interval` enum in the database. Only fixed-length
/// intervals: a month is not a number of seconds, so bars of that size cannot
/// be laid on a grid or folded into one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, sqlx::Type)]
#[sqlx(type_name = "bar_interval")]
pub enum BarInterval {
    #[sqlx(rename = "1m")]
    Min1,
    #[sqlx(rename = "2m")]
    Min2,
    #[sqlx(rename = "5m")]
    Min5,
    #[sqlx(rename = "15m")]
    Min15,
    #[sqlx(rename = "30m")]
    Min30,
    #[sqlx(rename = "1h")]
    Hour1,
    #[sqlx(rename = "1d")]
    Day1,
    #[sqlx(rename = "1wk")]
    Week1,
}

impl BarInterval {
    /// How many seconds one bar stands for.
    pub fn seconds(self) -> u64 {
        match self {
            Self::Min1 => 60,
            Self::Min2 => 120,
            Self::Min5 => 300,
            Self::Min15 => 900,
            Self::Min30 => 1_800,
            Self::Hour1 => 3_600,
            Self::Day1 => 86_400,
            Self::Week1 => 604_800,
        }
    }

    /// The start of the bar `ts` falls in, on a grid anchored at the epoch.
    ///
    /// The bar holding `ts` is still forming — nothing after `ts` has traded
    /// yet — so this is also the first moment a fetch up to `ts` cannot claim
    /// to have settled.
    pub fn floor(self, ts: Timestamp) -> Timestamp {
        let seconds = self.seconds();

        Timestamp::from_secs((ts.as_secs() / seconds) * seconds)
    }

    /// The word the database and the wire both use, `1m` through `1wk`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Min1 => "1m",
            Self::Min2 => "2m",
            Self::Min5 => "5m",
            Self::Min15 => "15m",
            Self::Min30 => "30m",
            Self::Hour1 => "1h",
            Self::Day1 => "1d",
            Self::Week1 => "1wk",
        }
    }

    /// The interval a wire word names, `None` for a word that is not one.
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "1m" => Some(Self::Min1),
            "2m" => Some(Self::Min2),
            "5m" => Some(Self::Min5),
            "15m" => Some(Self::Min15),
            "30m" => Some(Self::Min30),
            "1h" => Some(Self::Hour1),
            "1d" => Some(Self::Day1),
            "1wk" => Some(Self::Week1),
            _ => None,
        }
    }
}

/// Write what a fetch served: its bars, and the stretch of time it settles.
///
/// The two land in one transaction, because they are one claim. `covered` is
/// about knowledge rather than points — a source that answers a week with
/// three bars is saying the other days had no trading, and that is worth
/// storing. Any span it meets or overlaps is swallowed into one, so what the
/// table holds for a series stays sorted and disjoint, and `t0..t1` followed
/// by `t1..t2` reads back as `t0..t2`.
///
/// A bar already held is overwritten, which is what lets an adjusted series
/// land on top of the old one.
pub async fn save_bars(
    db: &Db,
    source: BarSource,
    symbol: &str,
    interval: BarInterval,
    bars: &[Bar],
    covered: Range<Timestamp>,
) -> Result<(), PostgresError> {
    let mut tx = db.begin().await?;

    if !bars.is_empty() {
        let ts: Vec<_> = bars.iter().map(|bar| stamp(bar.ts)).collect();
        let opens: Vec<Option<f64>> = bars.iter().map(|bar| bar.open).collect();
        let highs: Vec<Option<f64>> = bars.iter().map(|bar| bar.high).collect();
        let lows: Vec<Option<f64>> = bars.iter().map(|bar| bar.low).collect();
        let closes: Vec<f64> = bars.iter().map(|bar| bar.close).collect();
        let volumes: Vec<i64> = bars.iter().map(|bar| bar.volume as i64).collect();

        sqlx::query!(
            r#"INSERT INTO bars (source, symbol, interval, ts, open, high, low, close, volume)
               SELECT $1, $2, $3, ts, open, high, low, close, volume
               FROM UNNEST($4::timestamptz[], $5::float8[], $6::float8[], $7::float8[],
                           $8::float8[], $9::int8[])
                    AS t(ts, open, high, low, close, volume)
               ON CONFLICT (source, symbol, interval, ts) DO UPDATE SET
                   open   = EXCLUDED.open,
                   high   = EXCLUDED.high,
                   low    = EXCLUDED.low,
                   close  = EXCLUDED.close,
                   volume = EXCLUDED.volume"#,
            source as BarSource,
            symbol,
            interval as BarInterval,
            &ts,
            &opens as &[Option<f64>],
            &highs as &[Option<f64>],
            &lows as &[Option<f64>],
            &closes,
            &volumes,
        )
        .execute(&mut *tx)
        .await?;
    }

    if covered.start < covered.end {
        let mut merged_start = stamp(covered.start);
        let mut merged_end = stamp(covered.end);

        // Touching counts as overlapping: [0, 5) and [5, 9) is [0, 9). The
        // rows are locked so two writes to one series merge one after the
        // other rather than each against what the other is about to replace.
        let rows = sqlx::query!(
            r#"SELECT span_start, span_end FROM bar_coverage
               WHERE source = $1 AND symbol = $2 AND interval = $3
                 AND span_start <= $5 AND span_end >= $4
               FOR UPDATE"#,
            source as BarSource,
            symbol,
            interval as BarInterval,
            merged_start,
            merged_end,
        )
        .fetch_all(&mut *tx)
        .await?;

        for row in &rows {
            merged_start = merged_start.min(row.span_start);
            merged_end = merged_end.max(row.span_end);
        }

        sqlx::query!(
            r#"DELETE FROM bar_coverage
               WHERE source = $1 AND symbol = $2 AND interval = $3
                 AND span_start <= $5 AND span_end >= $4"#,
            source as BarSource,
            symbol,
            interval as BarInterval,
            merged_start,
            merged_end,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"INSERT INTO bar_coverage (source, symbol, interval, span_start, span_end)
               VALUES ($1, $2, $3, $4, $5)"#,
            source as BarSource,
            symbol,
            interval as BarInterval,
            merged_start,
            merged_end,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

/// The stretches of time a series is known over, sorted and disjoint.
///
/// Empty is a series nobody has fetched, which is not the same as one whose
/// window held no points.
pub async fn load_coverage(
    db: &Db,
    source: BarSource,
    symbol: &str,
    interval: BarInterval,
) -> Result<Vec<Range<Timestamp>>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT span_start, span_end FROM bar_coverage
           WHERE source = $1 AND symbol = $2 AND interval = $3
           ORDER BY span_start"#,
        source as BarSource,
        symbol,
        interval as BarInterval,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| seconds(row.span_start)..seconds(row.span_end))
        .collect())
}

/// The bars a series holds over `window`, in time order.
///
/// Half-open like coverage: a bar starting exactly at `window.end` is the
/// next window's. What the points mean — settled, or never fetched — is
/// [`load_coverage`]'s answer, not this one's.
pub async fn load_bars(
    db: &Db,
    source: BarSource,
    symbol: &str,
    interval: BarInterval,
    window: Range<Timestamp>,
) -> Result<Vec<Bar>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT ts, open, high, low, close, volume FROM bars
           WHERE source = $1 AND symbol = $2 AND interval = $3
             AND ts >= $4 AND ts < $5
           ORDER BY ts"#,
        source as BarSource,
        symbol,
        interval as BarInterval,
        stamp(window.start),
        stamp(window.end),
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Bar {
            ts: seconds(row.ts),
            open: row.open,
            high: row.high,
            low: row.low,
            close: row.close,
            volume: row.volume.max(0) as u64,
        })
        .collect())
}
