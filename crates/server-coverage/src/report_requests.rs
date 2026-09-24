//! The `/api/requests` answer: the newest asks of Yahoo, one row each.
//!
//! Newest first, so the first row is also what the page's Live dot reads —
//! a server asking right now has a row seconds old.

use axum::Json;
use axum::extract::Query;
use axum::extract::State;
use serde::Deserialize;
use serde::Serialize;

use crate::App;
use crate::Trouble;
use crate::unix_now;

/// How many rows the page gets when it does not say, and the most it may ask.
const DEFAULT_LIMIT: i64 = 100;
const MOST_LIMIT: i64 = 500;

#[derive(Deserialize)]
pub struct Ask {
    limit: Option<i64>,
}

/// The newest asks, and the moment they were read.
#[derive(Serialize)]
pub struct RequestsReport {
    now: u64,
    requests: Vec<RequestReport>,
}

/// One ask of the source.
#[derive(Serialize)]
struct RequestReport {
    at: u64,
    symbol: String,
    interval: String,
    start: u64,
    end: u64,
    outcome: String,
}

pub async fn serve(
    State(app): State<App>,
    Query(ask): Query<Ask>,
) -> Result<Json<RequestsReport>, Trouble> {
    let limit = ask.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MOST_LIMIT);

    let rows = src_postgres::load_recent_fetches(&app.db, limit).await?;

    Ok(Json(RequestsReport {
        now: unix_now().as_secs(),
        requests: rows
            .into_iter()
            .map(|row| RequestReport {
                at: row.at.as_secs(),
                symbol: row.symbol,
                interval: row.interval.as_str().to_string(),
                start: row.span.start.as_secs(),
                end: row.span.end.as_secs(),
                outcome: row.outcome.as_str().to_string(),
            })
            .collect(),
    }))
}
