//! The `/api/coverage/{interval}` answer: every Yahoo series at one interval,
//! with its spans, its edges and a curve thin enough to draw.
//!
//! The page does the geometry — what is missing, what is unavailable, what
//! percentage that makes — from the raw spans, so the server stays a reader.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::Serialize;
use src_postgres::BarInterval;
use src_postgres::BarSource;

use crate::App;
use crate::Trouble;
use crate::unix_now;

/// How many points a curve is thinned to before it travels.
const CURVE_POINTS: u32 = 240;

/// How many daily bars the turnover average reads — about a month of
/// sessions, enough to shrug off one heavy morning.
const TURNOVER_DAYS: u32 = 30;

/// One interval's coverage, whole.
#[derive(Serialize)]
pub struct CoverageReport {
    interval: String,
    /// Seconds one bar of this interval stands for.
    step: u64,
    since: u64,
    now: u64,
    /// Before this moment the source serves nothing at this interval;
    /// absent when its history has no floor.
    floor_start: Option<u64>,
    symbols: Vec<SymbolReport>,
}

/// One series: what is vouched for, what is held, and its shape.
#[derive(Serialize, Default)]
struct SymbolReport {
    symbol: String,
    name: Option<String>,
    /// Dollars a day, averaged over the newest month of daily bars.
    turnover: Option<f64>,
    bars: u64,
    first: Option<u64>,
    last: Option<u64>,
    spans: Vec<(u64, u64)>,
    curve: Vec<(u64, f64)>,
}

/// How far back Yahoo serves each interval, as
/// `server-main`'s `state_markets::history_floor` states it — that function
/// is the planner's word and this one is the page's, so a change there is a
/// change here.
fn history_floor(interval: BarInterval) -> Option<u64> {
    const DAY: u64 = 86_400;

    match interval {
        BarInterval::Min1 => Some(6 * DAY),
        BarInterval::Min2
        | BarInterval::Min5
        | BarInterval::Min15
        | BarInterval::Min30
        | BarInterval::Hour1 => Some(59 * DAY),
        BarInterval::Day1 | BarInterval::Week1 => None,
    }
}

pub async fn serve(
    State(app): State<App>,
    Path(word): Path<String>,
) -> Result<Json<CoverageReport>, Trouble> {
    let Some(interval) = BarInterval::parse(&word) else {
        return Err(Trouble::bad("no such interval"));
    };

    let now = unix_now();
    let source = BarSource::Yahoo;

    let spans = src_postgres::load_coverage_spans(&app.db, source, interval).await?;
    let stats = src_postgres::load_series_stats(&app.db, source, interval, app.since).await?;
    let heads = src_postgres::load_asset_heads(&app.db, source).await?;
    let turnovers = src_postgres::load_turnovers(&app.db, source, TURNOVER_DAYS).await?;

    let mut symbols: BTreeMap<String, SymbolReport> = BTreeMap::new();

    for row in spans {
        let entry = symbols.entry(row.symbol.clone()).or_default();
        entry.symbol = row.symbol;
        entry
            .spans
            .push((row.span.start.as_secs(), row.span.end.as_secs()));
    }

    for row in stats {
        let entry = symbols.entry(row.symbol.clone()).or_default();
        entry.symbol = row.symbol;
        entry.bars = row.bars;
        entry.first = Some(row.first.as_secs());
        entry.last = Some(row.last.as_secs());
    }

    let heads: BTreeMap<String, src_postgres::AssetHead> = heads
        .into_iter()
        .map(|head| (head.symbol.clone(), head))
        .collect();

    let turnovers: BTreeMap<String, f64> = turnovers
        .into_iter()
        .map(|row| (row.symbol, row.turnover))
        .collect();

    for (symbol, entry) in &mut symbols {
        if let Some(head) = heads.get(symbol) {
            entry.name = head.name.clone();
        }
        entry.turnover = turnovers.get(symbol).copied();

        if entry.bars > 0 {
            entry.curve = src_postgres::load_closes_thinned(
                &app.db,
                source,
                symbol,
                interval,
                app.since,
                CURVE_POINTS,
            )
            .await?
            .into_iter()
            .map(|point| (point.ts.as_secs(), point.close))
            .collect();
        }
    }

    Ok(Json(CoverageReport {
        interval: interval.as_str().to_string(),
        step: interval.seconds(),
        since: app.since.as_secs(),
        now: now.as_secs(),
        floor_start: history_floor(interval).map(|depth| now.saturating_sub_secs(depth).as_secs()),
        symbols: symbols.into_values().collect(),
    }))
}
