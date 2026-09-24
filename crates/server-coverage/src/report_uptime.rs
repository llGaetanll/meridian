//! The `/api/uptime` answer: two weeks of asking Yahoo, by the hour.
//!
//! Read from `fetch_log`, which only a server new enough to write it fills.
//! Against an older database the answer says so instead of failing, and the
//! page draws the absence.

use axum::Json;
use axum::extract::Query;
use axum::extract::State;
use serde::Deserialize;
use serde::Serialize;

use crate::App;
use crate::unix_now;

/// The window when the page does not say, and the most it may ask for.
const DEFAULT_HOURS: u64 = 24;
const MOST_HOURS: u64 = 14 * 24;

#[derive(Deserialize)]
pub struct Ask {
    hours: Option<u64>,
}

/// Two weeks of asking, counted and named.
#[derive(Serialize)]
pub struct UptimeReport {
    /// False when the database has no fetch log to read.
    present: bool,
    since: u64,
    now: u64,
    /// Hours with at least one ask: `(hour start, asked, faulted)`.
    hours: Vec<(u64, u64, u64)>,
    faults: Vec<FaultReport>,
    asked: u64,
    ok: u64,
}

/// One ask that ended badly.
#[derive(Serialize)]
struct FaultReport {
    at: u64,
    symbol: String,
    interval: String,
    outcome: String,
}

pub async fn serve(State(app): State<App>, Query(ask): Query<Ask>) -> Json<UptimeReport> {
    let hours = ask.hours.unwrap_or(DEFAULT_HOURS).clamp(1, MOST_HOURS);
    let now = unix_now();
    let since = now.saturating_sub_secs(hours * 3_600);

    match build(&app, since, now).await {
        Ok(report) => Json(report),
        Err(e) => {
            tracing::warn!(error = %e, "the fetch log cannot be read; drawing nothing");

            Json(UptimeReport {
                present: false,
                since: since.as_secs(),
                now: now.as_secs(),
                hours: Vec::new(),
                faults: Vec::new(),
                asked: 0,
                ok: 0,
            })
        }
    }
}

async fn build(
    app: &App,
    since: api_common::Timestamp,
    now: api_common::Timestamp,
) -> Result<UptimeReport, src_postgres::PostgresError> {
    let hours = src_postgres::load_fetch_hours(&app.db, since).await?;
    let totals = src_postgres::load_fetch_totals(&app.db, since).await?;
    let faults = src_postgres::load_fetch_faults(&app.db, since).await?;

    Ok(UptimeReport {
        present: true,
        since: since.as_secs(),
        now: now.as_secs(),
        hours: hours
            .into_iter()
            .map(|hour| (hour.hour.as_secs(), hour.asked, hour.faulted))
            .collect(),
        faults: faults
            .into_iter()
            .map(|fault| FaultReport {
                at: fault.at.as_secs(),
                symbol: fault.symbol,
                interval: fault.interval.as_str().to_string(),
                outcome: fault.outcome.as_str().to_string(),
            })
            .collect(),
        asked: totals.asked,
        ok: totals.ok,
    })
}
