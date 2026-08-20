//! The IO half of the server: everything effectful, and nothing that decides.
//!
//! The pairing `docs/ARCHITECTURE.md` describes — a state machine that decides
//! and a runtime that acts. The function structure mirrors the machine's:
//! `perform_actions` fans out to `perform_action`, which fans out to one
//! function per action group. Reading the two side by side, an action and the
//! thing that carries it out sit at the same depth.
//!
//! Only [`RuntimeServer::run`] is public. Everything else is reachable solely
//! by carrying out an action, which is what stops a caller from doing IO
//! without an action asking for it.

use std::sync::Arc;
use std::sync::RwLock;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use api_common::Timestamp;
use axum::Json;
use axum::Router;
use axum::extract::Query;
use axum::extract::State;
use axum::routing::get;
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;

use crate::Action;
use crate::ActionClient;
use crate::ActionProcess;
use crate::ActionSource;
use crate::Event;
use crate::EventClient;
use crate::EventSource;
use crate::EventTerminal;
use crate::StateServer;
use crate::runtime_plaid;
use crate::runtime_plaid::PlaidConfig;
use server_wire::WireSnapshot;
use server_wire::WireSource;

/// A different port from either front-end, so all three can run at once.
const PORT: u16 = 8082;

pub struct RuntimeServer {
    /// The machine.
    ///
    /// An `RwLock` rather than a `Mutex`: reads are the common case — every
    /// request that finds fresh data only projects a snapshot — and they can
    /// happen concurrently. Writes are the fetch path, which is rare.
    ///
    /// `handle_events` needs `&mut`, so a write is still exclusive. What this
    /// buys over a `Mutex` is that the many requests served from fresh state
    /// no longer queue behind each other. See the note in `docs/DECISIONS.md`
    /// about where this goes next.
    state: RwLock<StateServer>,

    /// Credentials, if there are any. Absent means the fixtures stand in.
    plaid: Option<PlaidConfig>,

    /// Only one fetch may be in flight. Without this, a burst of cold requests
    /// all miss the cache and all call Plaid.
    ///
    /// A `tokio::sync::Mutex` rather than a `std` one: the guard is held across
    /// the `.await` on Plaid, which a blocking guard cannot be.
    fetching: Mutex<()>,

    /// Raised once the machine asks to shut down.
    shutdown: Notify,
}

/// Everything a request can ask for.
#[derive(Debug, Default, Deserialize)]
struct SnapshotQuery {
    /// Skip the freshness window and go straight to Plaid. Pull-to-refresh.
    #[serde(default)]
    refresh: bool,
}

impl RuntimeServer {
    /// Build the server, wire the signal handler, and serve until told to stop.
    ///
    /// The only public entry point.
    pub async fn run() -> anyhow::Result<()> {
        let plaid = PlaidConfig::from_env();

        match &plaid {
            Some(_) => {
                tracing::info!(target: "app", source = "plaid", "serving live sandbox data")
            }
            None => tracing::info!(
                target: "app",
                source = "fixtures",
                "no PLAID_ACCESS_TOKEN, serving captured sandbox fixtures",
            ),
        }

        let runtime = Arc::new(Self {
            state: RwLock::new(StateServer::new()),
            plaid,
            fetching: Mutex::new(()),
            shutdown: Notify::new(),
        });

        let router = Router::new()
            .route("/snapshot", get(snapshot))
            .route("/health", get(health))
            // The phone app is served by trunk on another port, so every
            // request it makes is cross-origin. This is a development server on
            // localhost; a deployed one would name its origin instead.
            .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any))
            .with_state(Arc::clone(&runtime));

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", PORT)).await?;

        tracing::info!(target: "app", port = PORT, "server listening");

        let signals = Arc::clone(&runtime);

        axum::serve(listener, router)
            .with_graceful_shutdown(async move { signals.await_shutdown().await })
            .await?;

        tracing::info!(target: "app", "server stopped");

        Ok(())
    }

    /// Feed events in, carry out what comes back.
    ///
    /// The loop the architecture describes, one turn at a time: a request is
    /// the event source here rather than a `loop`, because axum owns the outer
    /// loop and hands us one batch per connection.
    async fn dispatch(&self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }

        let now = unix_now();
        let mut actions = Vec::new();

        {
            let mut state = self.write_state();
            state.handle_events(now, &events, &mut actions);
        }

        tracing::debug!(target: "app", ?events, ?actions, "handled");

        self.perform_actions(&actions).await;
    }

    /// Boxed because the cycle `perform_actions -> fetch_plaid ->
    /// perform_actions` is recursion through an `async fn`, which needs an
    /// indirection to have a finite size. The recursion is one level deep in
    /// practice: a fetch result only ever asks to serve.
    fn perform_actions<'a>(
        &'a self,
        actions: &'a [Action],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            for action in actions {
                self.perform_action(action).await;
            }
        })
    }

    async fn perform_action(&self, action: &Action) {
        match action {
            Action::ToSource { action } => self.perform_action_source(action).await,
            Action::ToProcess { action } => self.perform_action_process(action),
            // Serving is the response to the request that is already in hand,
            // so it is the absence of work here rather than a step: the handler
            // projects the snapshot once every action has been carried out.
            Action::ToClient {
                action: ActionClient::Serve,
            } => {}
        }
    }

    async fn perform_action_source(&self, action: &ActionSource) {
        match action {
            ActionSource::FetchPlaid { cursor } => self.fetch_plaid(cursor.clone()).await,
        }
    }

    fn perform_action_process(&self, action: &ActionProcess) {
        match action {
            ActionProcess::Shutdown => self.shutdown.notify_waiters(),

            ActionProcess::ReportLoginRequired => tracing::warn!(
                target: "app",
                "a connection needs the user to sign in again; syncs will fail until they do",
            ),
        }
    }

    /// Go to Plaid, and feed whatever comes back in as the next event.
    ///
    /// Nothing here inspects the answer or decides anything about it; that is
    /// what makes the loop closed.
    async fn fetch_plaid(&self, cursor: Option<String>) {
        // One fetch at a time. A burst of cold requests would otherwise all
        // miss and all call out.
        let Ok(_guard) = self.fetching.try_lock() else {
            tracing::debug!(target: "app", "a fetch is already in flight");
            return;
        };

        let event = match &self.plaid {
            Some(config) => match runtime_plaid::fetch_all(config, cursor, unix_now()).await {
                Ok(fetched) => EventSource::PlaidLoaded {
                    added: fetched.added,
                    modified: fetched.modified,
                    removed: fetched.removed,
                    connections: fetched.connections,
                    streams: fetched.streams,
                    cursor: fetched.cursor,
                },

                Err(e) => {
                    let code = match &e {
                        src_plaid::PlaidError::Plaid { error_code, .. } => error_code.clone(),
                        _ => "REQUEST_FAILED".to_string(),
                    };

                    tracing::warn!(target: "app", error = %e, code, "plaid fetch failed");

                    EventSource::PlaidFailed { code }
                }
            },

            // No credentials: the fixtures stand in, and the app runs anyway.
            None => {
                let fetched = runtime_plaid::fetch_fixtures(unix_now());

                EventSource::PlaidLoaded {
                    added: fetched.added,
                    modified: fetched.modified,
                    removed: fetched.removed,
                    connections: fetched.connections,
                    streams: fetched.streams,
                    cursor: fetched.cursor,
                }
            }
        };

        let now = unix_now();
        let mut actions = Vec::new();

        {
            let mut state = self.write_state();
            state.handle_events(now, &[Event::FromSource { event }], &mut actions);
        }

        // A fetch result only ever asks to serve, so there is nothing further
        // to carry out. Asserted rather than assumed: a new action here would
        // otherwise be dropped silently.
        debug_assert!(
            actions
                .iter()
                .all(|a| matches!(a, Action::ToClient { .. } | Action::ToProcess { .. })),
            "a fetch result should not ask for another fetch",
        );

        self.perform_actions(&actions).await;
    }

    /// Everything the app knows, as of now.
    ///
    /// The guard is taken and dropped inside this function rather than held by
    /// the caller: an `RwLockReadGuard` alive across an `.await` makes the
    /// handler's future `!Send`, which axum will not accept.
    fn project_snapshot(&self) -> WireSnapshot {
        let source = if self.plaid.is_some() {
            WireSource::Live
        } else {
            WireSource::Fixture
        };

        let now = unix_now();
        let state = self.read_state();

        WireSnapshot::new(
            state.entries(),
            state.connections(),
            state.streams(),
            now,
            source,
        )
    }

    /// Wait for the machine to ask for a shutdown, having first turned the
    /// signal into an event.
    async fn await_shutdown(&self) {
        let notified = self.shutdown.notified();

        tokio::select! {
            event = signal_received() => {
                // The signal is not the decision. The machine gets to say what
                // happens next, which is what makes shutdown replayable.
                self.dispatch(vec![Event::FromTerminal { event }]).await;
            }
            _ = notified => {}
        }

        tracing::info!(target: "app", "shutting down");
    }

    /// A poisoned lock means another thread panicked while holding it.
    ///
    /// The server carries on with the held data — losing every future request
    /// is worse than serving one that may be mid-update — but it says so, at
    /// `ERROR`, every time. Recovering silently would turn a panic somewhere
    /// else into a mystery here, which is exactly what `docs/LOGGING.md` exists
    /// to prevent.
    fn read_state(&self) -> std::sync::RwLockReadGuard<'_, StateServer> {
        match self.state.read() {
            Ok(state) => state,
            Err(poisoned) => {
                tracing::error!(
                    target: "app",
                    "server state lock is poisoned — a thread panicked while holding it; \
                     carrying on with the held data",
                );

                poisoned.into_inner()
            }
        }
    }

    fn write_state(&self) -> std::sync::RwLockWriteGuard<'_, StateServer> {
        match self.state.write() {
            Ok(state) => state,
            Err(poisoned) => {
                tracing::error!(
                    target: "app",
                    "server state lock is poisoned — a thread panicked while holding it; \
                     carrying on with the held data",
                );

                poisoned.into_inner()
            }
        }
    }
}

/// `GET /snapshot`
async fn snapshot(
    State(runtime): State<Arc<RuntimeServer>>,
    Query(query): Query<SnapshotQuery>,
) -> Json<WireSnapshot> {
    runtime
        .dispatch(vec![Event::FromClient {
            event: EventClient::SnapshotRequested {
                force: query.refresh,
            },
        }])
        .await;

    Json(runtime.project_snapshot())
}

/// `GET /health`
async fn health() -> &'static str {
    "ok"
}

/// Resolves with whichever signal arrives first.
async fn signal_received() -> EventTerminal {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            // Without a handler there is nothing to wait for; ^C still works.
            Err(e) => {
                tracing::warn!(target: "app", error = %e, "could not listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = interrupt => EventTerminal::SigInt,
        _ = terminate => EventTerminal::SigTerm,
    }
}

/// The wall clock, read here and nowhere below. The state machine takes it as
/// an argument so that a replay is not at the mercy of when it happens to run.
fn unix_now() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| Timestamp::from_secs(d.as_secs()))
        .unwrap_or(Timestamp::EPOCH)
}
