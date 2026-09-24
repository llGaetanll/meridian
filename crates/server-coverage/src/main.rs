//! The coverage page: how much of the market data we mean to hold is held.
//!
//! An internal tool rather than a part of the app. It binds to localhost,
//! reads whatever database it is pointed at and writes nothing — pointed at
//! prod through the tunnel it draws prod, pointed at a laptop it draws that.
//! It queries per request, which the app's server must never do: nobody
//! replays this page, and here stale is worse than slow.
//!
//! `DATABASE_URL` says where the database is. `COVERAGE_ADDR` moves the page
//! off `127.0.0.1:8090`. Each series is counted and drawn from wherever it
//! begins; `COVERAGE_SINCE`, a `YYYY-MM-DD` day, cuts them all off at a
//! chosen horizon instead.

mod report_coverage;
mod report_requests;
mod report_uptime;

use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use api_common::Timestamp;
use axum::Router;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use src_postgres::Db;

/// Everything a handler reaches for.
#[derive(Clone)]
struct App {
    db: Db,
    /// The horizon: nothing before it is counted, drawn or asked about.
    since: Timestamp,
}

/// A read that broke, carried to the page as text and a status.
struct Trouble {
    status: StatusCode,
    message: String,
}

impl Trouble {
    fn bad(message: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.to_string(),
        }
    }
}

impl From<src_postgres::PostgresError> for Trouble {
    fn from(e: src_postgres::PostgresError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        }
    }
}

impl IntoResponse for Trouble {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

/// The wall clock. The app's server takes time as an event; this tool has no
/// machine to replay, so reading the clock where it is needed is fine here.
fn unix_now() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| Timestamp::from_secs(d.as_secs()))
        .unwrap_or(Timestamp::EPOCH)
}

/// The horizon from `COVERAGE_SINCE`, a `YYYY-MM-DD` day read as midnight
/// UTC. Unset means the epoch: each series is then counted and drawn from
/// wherever it begins.
fn horizon() -> anyhow::Result<Timestamp> {
    let Ok(word) = std::env::var("COVERAGE_SINCE") else {
        return Ok(Timestamp::EPOCH);
    };

    let day = chrono::NaiveDate::parse_from_str(&word, "%Y-%m-%d")
        .with_context(|| format!("COVERAGE_SINCE is not a YYYY-MM-DD day: {word}"))?;

    let midnight = day
        .and_hms_opt(0, 0, 0)
        .context("midnight exists on every day")?;

    Ok(Timestamp::from_secs(
        midnight.and_utc().timestamp().max(0) as u64
    ))
}

async fn page() -> impl IntoResponse {
    axum::response::Html(include_str!("../assets/index.html"))
}

/// The same roles the sketches and the app read, from the one file they are
/// decided in.
async fn theme() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../../../sketches/shared/theme.css"),
    )
}

async fn controls() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../../../sketches/shared/app.css"),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    api_trace::init(api_trace::Config {
        quiet: &["sqlx::postgres::notice"],
        file: None,
        ..api_trace::Config::default()
    });

    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let db = src_postgres::connect(&url).await?;
    let since = horizon()?;

    let addr = std::env::var("COVERAGE_ADDR").unwrap_or_else(|_| "127.0.0.1:8090".to_string());

    let app = Router::new()
        .route("/", get(page))
        .route("/theme.css", get(theme))
        .route("/app.css", get(controls))
        .route("/api/coverage/{interval}", get(report_coverage::serve))
        .route("/api/requests", get(report_requests::serve))
        .route("/api/uptime", get(report_uptime::serve))
        .with_state(App { db, since });

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;

    tracing::info!(addr, "the coverage page is up");

    axum::serve(listener, app).await?;

    Ok(())
}
