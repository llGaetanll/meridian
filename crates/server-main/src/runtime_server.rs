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

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use api_common::Timestamp;
use axum::Json;
use axum::Router;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
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
use crate::ActionStore;
use crate::Event;
use crate::EventClient;
use crate::EventProcess;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;
use crate::PlaidCollect;
use crate::StateServer;
use crate::project;
use crate::runtime_plaid;
use server_wire::WireSnapshot;
use src_plaid::PlaidCredentials;
use src_postgres::Db;
use uuid::Uuid;

/// A different port from either front-end, so all three can run at once.
const PORT: u16 = 8082;

pub struct RuntimeServer {
    /// The store. Required, and not optional in any sense: the access tokens
    /// live in it, so a server without one has no way to reach any bank and
    /// nothing to serve. It used to be optional and hold state in memory,
    /// which was the last variable in the server with a fallback behind it.
    db: Db,

    /// The machine.
    ///
    /// An `RwLock` rather than a `Mutex`: reads are the common case — every
    /// request that finds fresh data only projects a snapshot — and they can
    /// happen concurrently. Writes are the fetch path, which is rare.
    ///
    /// `handle_events` needs `&mut`, so a write is still exclusive. What this
    /// buys over a `Mutex` is that the many requests served from fresh state no
    /// longer queue behind each other.
    ///
    /// If write contention ever becomes the bottleneck, the next step is
    /// interior mutability inside the machine rather than a bigger lock out
    /// here — but that trades one contended lock for a machine that can no
    /// longer be read as a plain `&mut self` function, and the blocking work is
    /// the Plaid fetch, which `fetching` already serialises.
    state: RwLock<StateServer>,

    /// The Plaid account these calls are made on behalf of. One per process;
    /// it says which environment, and it signs every request.
    credentials: PlaidCredentials,

    /// Whose data this server serves.
    ///
    /// **A scaffold.** The database is multi-user and every query is scoped,
    /// but nothing on a request says who is asking yet — that is the sessions
    /// table and the auth slice. Until then one user is named by
    /// `MERIDIAN_USER_EMAIL` at startup, which is a stand-in that is at least
    /// an honest one: it is required, it is logged, and it is a real row.
    user_id: Uuid,

    /// Every Item's access token, by `item_id`, read from the store at boot
    /// and added to as banks are linked.
    ///
    /// Here rather than in the machine, and deliberately: a token is a live
    /// credential to somebody's bank account, and the machine's whole job is
    /// to be logged and replayed. It never sees one. Actions name an Item and
    /// the runtime looks the token up, which is why `ActionSource::FetchPlaid`
    /// carries an `item_id`.
    tokens: RwLock<HashMap<String, String>>,

    /// Which channel collects the result of a Link session. Held here because
    /// the link flow is not built yet; when it is, this becomes a fact on
    /// `EventProcess::Started` and the machine decides what to do about it.
    /// Read and validated at startup regardless, so a misconfigured server
    /// says so on the way up rather than on the day somebody links a bank.
    ///
    /// `expect` rather than `allow`: the day the link flow reads this, the
    /// attribute itself starts warning and asks to be deleted.
    #[expect(
        dead_code,
        reason = "read once the link flow lands; validated and logged at startup meanwhile"
    )]
    collect: PlaidCollect,

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
        // All of these are required, and each fails the process rather than
        // degrading. Read before anything is served, so a server that cannot
        // work says so in its first second rather than on its first request.
        let credentials = PlaidCredentials::from_env()?;
        let collect = PlaidCollect::from_env()?;
        let db = Self::open_store().await?;

        let email = std::env::var("MERIDIAN_USER_EMAIL")
            .map_err(|_| anyhow::anyhow!("MERIDIAN_USER_EMAIL is not set"))?;

        let user_id = src_postgres::upsert_user(&db, &email).await?;

        tracing::info!(%user_id, "serving one user until sessions exist");

        tracing::info!(
            env = credentials.env.as_str(),
            collect = collect.as_str(),
            "serving live plaid data",
        );

        let runtime = Arc::new(Self {
            state: RwLock::new(StateServer::new()),
            credentials,
            user_id,
            tokens: RwLock::new(HashMap::new()),
            collect,
            db,
            fetching: Mutex::new(()),
            shutdown: Notify::new(),
        });

        // The first event, and the reason the boot sequence is replayable: the
        // machine asks to be filled from the store rather than the runtime
        // deciding to do it on its own initiative.
        runtime
            .dispatch(vec![Event::FromProcess {
                event: EventProcess::Started,
            }])
            .await;

        let router = Router::new()
            .route("/snapshot", get(snapshot))
            .route("/health", get(health))
            // The phone app is served by trunk on another port, so every
            // request it makes is cross-origin. This is a development server on
            // localhost; a deployed one would name its origin instead.
            .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any))
            .with_state(Arc::clone(&runtime));

        // Every interface rather than loopback, because a phone on the LAN is
        // one of the clients. The simulator shares the Mac's loopback and would
        // be happy either way; a real device has to reach the Mac by its
        // address on the network, and nothing answers there if we bind
        // loopback. This is a development server on a home network.
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", PORT)).await?;

        tracing::info!(port = PORT, "server listening");

        let signals = Arc::clone(&runtime);

        axum::serve(listener, router)
            .with_graceful_shutdown(async move { signals.await_shutdown().await })
            .await?;

        tracing::info!("server stopped");

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

        tracing::debug!(?events, ?actions, "handled");

        self.perform_actions(&actions).await;
    }

    /// Connect to Postgres and bring the schema up to date, if there is one to
    /// connect to.
    ///
    /// Migrations run here, on boot, rather than as a step in `docker compose`
    /// or a script somebody has to remember. That is what makes bringing the
    /// whole thing up one command: the binary knows which schema it needs and
    /// says so to whatever database it is handed.
    ///
    /// A `DATABASE_URL` that is missing or wrong is fatal, deliberately. This
    /// is where the access tokens live, so a server without a store cannot
    /// reach a bank at all — and one that started anyway would look healthy
    /// while quietly keeping nothing.
    async fn open_store() -> anyhow::Result<Db> {
        let url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL is not set"))?;

        let db = src_postgres::connect(&url).await?;

        src_postgres::migrate(&db).await?;

        tracing::info!(store = "postgres", "store ready");

        Ok(db)
    }

    /// Boxed because the cycle `perform_actions -> fetch_plaid ->
    /// perform_actions` is recursion through an `async fn`, which needs an
    /// indirection to have a finite size. The recursion is one level deep in
    /// practice: a fetch result asks to serve and to write, never to fetch
    /// again — which `fetch_plaid` asserts rather than assumes.
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
            Action::ToStore { action } => self.perform_action_store(action).await,
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
            ActionSource::FetchPlaid { item_id, cursor } => {
                self.fetch_plaid(item_id, cursor.clone()).await
            }
        }
    }

    /// Everything the store is asked for.
    ///
    /// Each arm ends by feeding the outcome back in as an event, which is what
    /// closes the loop: nothing here inspects an answer or decides anything
    /// about it.
    async fn perform_action_store(&self, action: &ActionStore) {
        let db = &self.db;

        match action {
            ActionStore::LoadEverything => {
                let event = match self.load_everything(db).await {
                    Ok(event) => event,
                    Err(e) => EventStore::LoadFailed {
                        message: e.to_string(),
                    },
                };

                self.dispatch(vec![Event::FromStore { event }]).await;
            }

            ActionStore::SaveFetched { .. } | ActionStore::SaveStatus { .. } => {
                if let Err(e) = self.write(db, action).await {
                    self.dispatch(vec![Event::FromStore {
                        event: EventStore::SaveFailed {
                            message: e.to_string(),
                        },
                    }])
                    .await;
                }
            }
        }
    }

    /// Read every Item back, and the ledger with it.
    ///
    /// The connections are assembled here rather than in the store, because a
    /// connection is a *projection* — initials, a status — and `src-postgres`
    /// returns records. The status rule is `project`'s, shared with the path
    /// that builds one from a fresh balances response.
    async fn load_everything(&self, db: &Db) -> Result<EventStore, src_postgres::PostgresError> {
        let items = src_postgres::load_items(db, self.user_id).await?;

        // The tokens go to the runtime and stop there. Everything below this
        // line is a projection for a screen, and none of it carries one.
        {
            let mut tokens = self.write_tokens();

            for item in &items {
                tokens.insert(item.item_id.clone(), item.access_token.clone());
            }
        }

        let mut connections = Vec::with_capacity(items.len());

        for item in &items {
            let accounts = src_postgres::load_accounts(db, &item.item_id).await?;

            let name = item
                .institution_name
                .clone()
                .or_else(|| item.institution_id.clone())
                .unwrap_or_else(|| "Bank".to_string());

            connections.push(api_ledger::Connection {
                id: item.item_id.clone(),
                initials: api_ledger::initials(&name),
                name,
                status: project::connection_status(item.error_code.as_deref(), item.synced_at),
                accounts,
            });
        }

        // One Item today, as `ITEM` in the machine says, which is why a single
        // cursor is a faithful answer rather than a choice made here. The day
        // that stops being true this is the line that says so.
        debug_assert!(
            items.len() <= 1,
            "a second Item needs per-Item state, not a cursor picked here",
        );

        Ok(EventStore::Loaded {
            entries: src_postgres::load_entries(db).await?,
            streams: src_postgres::load_streams(db).await?,
            // One Item today; the first is the one whose cursor a fetch
            // resumes from. See `ITEM` in the machine.
            cursor: items.first().and_then(|item| item.cursor.clone()),
            synced_at: items.first().map(|item| item.synced_at),
            connections,
        })
    }

    /// The writes, in the order a failure can be recovered from.
    ///
    /// The cursor goes **last**, on purpose. If anything before it fails, the
    /// cursor stays where it was and the next sync re-reads the same range —
    /// which is safe, because applying a diff twice is a no-op. Writing it
    /// first and then failing would skip that range forever.
    async fn write(
        &self,
        db: &Db,
        action: &ActionStore,
    ) -> Result<(), src_postgres::PostgresError> {
        match action {
            ActionStore::SaveFetched {
                item_id,
                institution_name,
                added,
                modified,
                removed,
                accounts,
                streams,
                cursor,
            } => {
                // The access token is the runtime's, not the machine's: it
                // never enters an action or the event log. Looked up rather
                // than carried, for the same reason.
                //
                // Absent means a sync landed for an Item nothing linked, which
                // cannot happen — the fetch that produced it needed the same
                // token. Reported rather than defaulted: an empty string here
                // would write a row claiming to be a bank connection with no
                // credential behind it, which is worse than no row at all.
                let Some(access_token) = self.token_for(item_id) else {
                    tracing::error!(item_id, "a sync landed for an item with no token");

                    return Ok(());
                };

                src_postgres::save_item(
                    db,
                    &src_postgres::StoredItem {
                        item_id: item_id.clone(),
                        user_id: self.user_id,
                        access_token,
                        institution_id: None,
                        institution_name: institution_name.clone(),
                        cursor: None,
                        error_code: None,
                        synced_at: unix_now(),
                    },
                )
                .await?;

                src_postgres::save_accounts(db, item_id, accounts).await?;
                src_postgres::apply_entries(db, item_id, added, modified, removed).await?;
                src_postgres::save_streams(db, item_id, streams).await?;
                src_postgres::save_cursor(db, item_id, cursor).await?;

                tracing::debug!(
                    item_id,
                    added = added.len(),
                    modified = modified.len(),
                    removed = removed.len(),
                    "written to the store",
                );
            }

            ActionStore::SaveStatus {
                item_id,
                error_code,
            } => {
                src_postgres::save_error(db, item_id, error_code.as_deref()).await?;
            }

            ActionStore::LoadEverything => {}
        }

        Ok(())
    }

    fn perform_action_process(&self, action: &ActionProcess) {
        match action {
            ActionProcess::Shutdown => self.shutdown.notify_waiters(),

            ActionProcess::ReportLoginRequired => tracing::warn!(
                "a connection needs the user to sign in again; syncs will fail until they do",
            ),
        }
    }

    /// Go to Plaid, and feed whatever comes back in as the next event.
    ///
    /// Nothing here inspects the answer or decides anything about it; that is
    /// what makes the loop closed.
    async fn fetch_plaid(&self, item_id: &str, cursor: Option<String>) {
        // One fetch at a time. A burst of cold requests would otherwise all
        // miss and all call out.
        let Ok(_guard) = self.fetching.try_lock() else {
            tracing::debug!("a fetch is already in flight");
            return;
        };

        // The machine named an Item it read off its own connection list, and
        // every connection there was built from a stored Item. Absent means
        // those two have drifted, which is worth saying rather than guessing
        // at.
        let Some(access_token) = self.token_for(item_id) else {
            tracing::error!(item_id, "asked to fetch an item with no token");

            self.dispatch(vec![Event::FromSource {
                event: EventSource::PlaidFailed {
                    code: String::from("NO_ACCESS_TOKEN"),
                },
            }])
            .await;

            return;
        };

        let event =
            match runtime_plaid::fetch_all(&self.credentials, &access_token, cursor, unix_now())
                .await
            {
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

                    tracing::warn!(error = %e, code, "plaid fetch failed");

                    EventSource::PlaidFailed { code }
                }
            };

        let now = unix_now();
        let mut actions = Vec::new();

        {
            let mut state = self.write_state();
            state.handle_events(now, &[Event::FromSource { event }], &mut actions);
        }

        // What this protects is the claim on `perform_actions`: the recursion
        // through it is one level deep. A fetch result may ask to serve, to
        // write, or to report — anything but to fetch again, which is the one
        // answer that would not terminate.
        //
        // Stated as what it forbids rather than as a list of what it allows.
        // The allowlist version passed until the store was added, and then
        // failed on an action that was perfectly fine.
        debug_assert!(
            !actions.iter().any(|a| matches!(a, Action::ToSource { .. })),
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
        let now = unix_now();
        let state = self.read_state();

        WireSnapshot::new(state.entries(), state.connections(), state.streams(), now)
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

        tracing::info!("shutting down");
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
                    "server state lock is poisoned — a thread panicked while holding it; \
                     carrying on with the held data",
                );

                poisoned.into_inner()
            }
        }
    }

    /// One Item's access token, if we hold one.
    fn token_for(&self, item_id: &str) -> Option<String> {
        match self.tokens.read() {
            Ok(tokens) => tokens.get(item_id).cloned(),
            Err(poisoned) => {
                tracing::error!(
                    "the token map lock is poisoned — a thread panicked while holding it; \
                     carrying on with the held data",
                );

                poisoned.into_inner().get(item_id).cloned()
            }
        }
    }

    fn write_tokens(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<String, String>> {
        match self.tokens.write() {
            Ok(tokens) => tokens,
            Err(poisoned) => {
                tracing::error!(
                    "the token map lock is poisoned — a thread panicked while holding it; \
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
/// Whether the server is up, and whether what it depends on is.
///
/// It reports the database by asking it something rather than by checking that
/// a pool exists: a pool holds connections the far end may have closed since,
/// so the only honest answer comes from a query. This is what `docker compose
/// up` is checked with, so an optimistic answer here would defeat the point of
/// having it.
async fn health(State(runtime): State<Arc<RuntimeServer>>) -> (StatusCode, String) {
    match src_postgres::ping(&runtime.db).await {
        Ok(()) => (StatusCode::OK, String::from("ok store=postgres")),
        Err(e) => {
            tracing::error!(error = %e, "the store is not answering");

            (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("store unreachable: {e}"),
            )
        }
    }
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
                tracing::warn!(error = %e, "could not listen for SIGTERM");
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
