//! The server, which exists because the phone cannot hold the Plaid secret.
//!
//! `docs/PLAID.md`: the secret grants full access to every Item on the
//! account, so it cannot ship inside an app anyone can unzip. Every call that
//! carries it happens here, and the only thing the phone ever sees is the
//! transactions that come back.
//!
//! **Sandbox only.** `PEnv` defaults to Sandbox and needs a deliberate
//! `PLAID_ENV=production` to be anything else; nothing here sets it, and
//! [`plaid_io::fetch_all`] asserts on the environment before it calls out.
//!
//! Run it with `cargo run -p srv-main`. With no credentials it serves the
//! captured Sandbox fixtures instead, so the phone app runs from a clean
//! checkout.

mod core;
mod plaid_io;
mod project;

use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use axum::Json;
use axum::Router;
use axum::extract::Query;
use axum::extract::State;
use axum::routing::get;
use serde::Deserialize;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;

use crate::core::ServerAction;
use crate::core::ServerEvent;
use crate::core::ServerState;
use crate::plaid_io::PlaidConfig;
use api_wire::Snapshot;
use api_wire::SnapshotSource;

/// A different port from either front-end, so all three can run at once.
const PORT: u16 = 8082;

struct App {
    state: Mutex<ServerState>,
    plaid: Option<PlaidConfig>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    api_trace::init(api_trace::Config::default());

    tracing::info!(
        target: "app",
        version = env!("CARGO_PKG_VERSION"),
        "meridian server starting",
    );

    let plaid = PlaidConfig::from_env();

    match &plaid {
        Some(_) => tracing::info!(target: "app", source = "plaid", "serving live sandbox data"),
        None => tracing::info!(
            target: "app",
            source = "fixtures",
            "no PLAID_ACCESS_TOKEN, serving captured sandbox fixtures",
        ),
    }

    let app = Arc::new(App {
        state: Mutex::new(ServerState::default()),
        plaid,
    });

    let router = Router::new()
        .route("/snapshot", get(snapshot))
        .route("/health", get(|| async { "ok" }))
        // The phone app is served by trunk on another port, so every request
        // it makes is cross-origin. This is a development server on
        // localhost; a deployed one would name its origin instead.
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any))
        .with_state(app);

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", PORT)).await?;

    tracing::info!(target: "app", port = PORT, "server listening");

    axum::serve(listener, router).await?;

    Ok(())
}

#[derive(Debug, Default, Deserialize)]
struct SnapshotQuery {
    /// Skip the freshness window and go straight to Plaid. This is what
    /// pull-to-refresh sends.
    #[serde(default)]
    refresh: bool,
}

/// Everything the app knows, as of now.
///
/// The handler is the runtime the architecture describes: it reads the clock,
/// hands an event to the machine, and carries out whatever actions come back.
/// It decides nothing itself.

async fn snapshot(
    State(app): State<Arc<App>>,
    Query(query): Query<SnapshotQuery>,
) -> Json<Snapshot> {
    let now = unix_now();

    let actions = {
        let mut state = app.state.lock().expect("server state mutex poisoned");
        let mut actions = Vec::new();

        state.handle_events(
            &[ServerEvent::SnapshotRequested {
                now,
                force: query.refresh,
            }],
            &mut actions,
        );

        actions
    };

    for action in actions {
        match action {
            ServerAction::FetchPlaid { cursor } => fetch(&app, cursor, now).await,
            // Nothing to do: the answer is assembled from state below either
            // way, so `Serve` is the absence of work rather than a step.
            ServerAction::Serve | ServerAction::MarkLoginRequired => {}
        }
    }

    let source = if app.plaid.is_some() {
        SnapshotSource::Live
    } else {
        SnapshotSource::Fixture
    };

    // Built inside the lock and returned after it, so the guard never has to
    // live across an await — which is what keeps this handler's future `Send`.
    let snapshot = {
        let state = app.state.lock().expect("server state mutex poisoned");

        Snapshot::new(
            state.entries(),
            state.connections(),
            state.streams(),
            now,
            source,
        )
    };

    Json(snapshot)
}

/// Carry out a `FetchPlaid`, and feed the result back in as the next event.
async fn fetch(app: &Arc<App>, cursor: Option<String>, now: u64) {
    let event = match &app.plaid {
        Some(config) => match plaid_io::fetch_all(config, cursor).await {
            Ok(fetched) => ServerEvent::PlaidLoaded {
                now,
                entries: fetched.entries,
                connections: fetched.connections,
                streams: fetched.streams,
                cursor: fetched.cursor,
            },
            Err(e) => {
                let code = match &e {
                    src_plaid::PError::Plaid { error_code, .. } => error_code.clone(),
                    _ => "REQUEST_FAILED".to_string(),
                };

                tracing::warn!(target: "app", error = %e, code, "plaid fetch failed");

                ServerEvent::PlaidFailed { now, code }
            }
        },

        // No credentials: the fixtures stand in, and the app runs anyway.
        None => {
            let fetched = plaid_io::fetch_fixtures();

            ServerEvent::PlaidLoaded {
                now,
                entries: fetched.entries,
                connections: fetched.connections,
                streams: fetched.streams,
                cursor: fetched.cursor,
            }
        }
    };

    let mut state = app.state.lock().expect("server state mutex poisoned");
    let mut actions = Vec::new();

    state.handle_events(&[event], &mut actions);

    // Nothing to carry out: the events above only ever ask to serve, and the
    // answer is assembled by the caller from the state just written.
    debug_assert!(
        actions
            .iter()
            .all(|a| matches!(a, ServerAction::Serve | ServerAction::MarkLoginRequired)),
        "a fetch result should not ask for another fetch",
    );
}

/// The wall clock, read here and nowhere below. The state machine takes it as
/// an argument so that a replay is not at the mercy of when it happens to run.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
