//! The record that a market fetch was made, and how it went.
//!
//! Written by the runtime as it carries the call out, never read by the
//! machine: the readers are the coverage page's uptime bar and request
//! counts. `bar_coverage` is what a fetch settled; this is that it happened.

use std::ops::Range;

use api_common::Timestamp;

use crate::Db;
use crate::PostgresError;
use crate::postgres_bars::BarInterval;
use crate::postgres_bars::BarSource;
use crate::postgres_ledger::seconds;
use crate::postgres_ledger::stamp;

/// How one ask of a source ended.
///
/// Mirrors the `fetch_outcome` enum in the database, and the three faults of
/// `YahooFault` plus the answer that was one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "fetch_outcome", rename_all = "snake_case")]
pub enum FetchOutcome {
    Ok,
    RateLimited,
    Refused,
    Failed,
}

impl FetchOutcome {
    /// The word the database holds, `ok` through `failed`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::RateLimited => "rate_limited",
            Self::Refused => "refused",
            Self::Failed => "failed",
        }
    }
}

/// One hour of asking, counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchHour {
    /// The start of the hour.
    pub hour: Timestamp,
    /// Asks made in it, however they ended.
    pub asked: u64,
    /// The asks among them that ended in anything but an answer.
    pub faulted: u64,
}

/// How the asking went over a window, in total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FetchTotals {
    pub asked: u64,
    pub ok: u64,
}

/// One ask that ended badly, for naming it on the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchFault {
    pub at: Timestamp,
    pub symbol: String,
    pub interval: BarInterval,
    pub outcome: FetchOutcome,
}

/// One ask, whole: when, what for, and how it went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchRecord {
    pub at: Timestamp,
    pub symbol: String,
    pub interval: BarInterval,
    pub span: Range<Timestamp>,
    pub outcome: FetchOutcome,
}

/// Write down one ask of a source.
pub async fn record_fetch(
    db: &Db,
    at: Timestamp,
    source: BarSource,
    symbol: &str,
    interval: BarInterval,
    span: Range<Timestamp>,
    outcome: FetchOutcome,
) -> Result<(), PostgresError> {
    sqlx::query!(
        r#"INSERT INTO fetch_log (at, source, symbol, interval, span_start, span_end, outcome)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        stamp(at),
        source as BarSource,
        symbol,
        interval as BarInterval,
        stamp(span.start),
        stamp(span.end),
        outcome as FetchOutcome,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// The newest asks, newest first, at most `limit` of them.
pub async fn load_recent_fetches(db: &Db, limit: i64) -> Result<Vec<FetchRecord>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT at, symbol,
                  interval AS "interval: BarInterval",
                  span_start, span_end,
                  outcome AS "outcome: FetchOutcome"
           FROM fetch_log
           ORDER BY at DESC
           LIMIT $1"#,
        limit.max(0),
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| FetchRecord {
            at: seconds(row.at),
            symbol: row.symbol,
            interval: row.interval,
            span: seconds(row.span_start)..seconds(row.span_end),
            outcome: row.outcome,
        })
        .collect())
}

/// The asking since `since`, counted by the hour it happened in. Hours with
/// no asks in them are absent rather than zero.
pub async fn load_fetch_hours(db: &Db, since: Timestamp) -> Result<Vec<FetchHour>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT date_trunc('hour', at) AS "hour!",
                  count(*) AS "asked!",
                  count(*) FILTER (WHERE outcome <> 'ok') AS "faulted!"
           FROM fetch_log
           WHERE at >= $1
           GROUP BY 1
           ORDER BY 1"#,
        stamp(since),
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| FetchHour {
            hour: seconds(row.hour),
            asked: row.asked.max(0) as u64,
            faulted: row.faulted.max(0) as u64,
        })
        .collect())
}

/// The asking since `since`, in total.
pub async fn load_fetch_totals(db: &Db, since: Timestamp) -> Result<FetchTotals, PostgresError> {
    let row = sqlx::query!(
        r#"SELECT count(*) AS "asked!",
                  count(*) FILTER (WHERE outcome = 'ok') AS "ok!"
           FROM fetch_log
           WHERE at >= $1"#,
        stamp(since),
    )
    .fetch_one(db)
    .await?;

    Ok(FetchTotals {
        asked: row.asked.max(0) as u64,
        ok: row.ok.max(0) as u64,
    })
}

/// Every ask since `since` that ended badly, in time order.
pub async fn load_fetch_faults(
    db: &Db,
    since: Timestamp,
) -> Result<Vec<FetchFault>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT at, symbol,
                  interval AS "interval: BarInterval",
                  outcome AS "outcome: FetchOutcome"
           FROM fetch_log
           WHERE at >= $1 AND outcome <> 'ok'
           ORDER BY at"#,
        stamp(since),
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| FetchFault {
            at: seconds(row.at),
            symbol: row.symbol,
            interval: row.interval,
            outcome: row.outcome,
        })
        .collect())
}
