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
use axum::http::StatusCode;
use tokio::sync::Mutex;
use tokio::sync::Notify;

use crate::Action;
use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionSource;
use crate::ActionStore;
use crate::EmailSend;
use crate::Event;
use crate::EventProcess;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;
use crate::PlaidCollect;
use crate::StateServer;
use crate::StoredCode;
use crate::project;
use crate::router_server;
use crate::runtime_plaid;
use crate::session_token;
use server_wire::WireSnapshot;
use src_plaid::PlaidCredentials;
use src_postgres::Db;
use src_resend::ResendClient;
use src_resend::ResendCredentials;
use uuid::Uuid;

/// A different port from either front-end, so all three can run at once.
const PORT: u16 = 8082;

/// How often the machine is handed fresh entropy.
///
/// Every seed is 32 bytes of ChaCha20 state, so one covers far more codes than
/// a minute will ever produce. The interval is not about running out — it is
/// about how much a single leaked seed is worth.
const RESEED_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

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

    /// How a sign-in code reaches the person asking for it.
    email_send: EmailSend,

    /// Resend, when [`RuntimeServer::email_send`] says to use it. `None` when
    /// it says `log`, and nothing consults it in that case.
    resend: Option<ResendClient>,

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

        let email_send = EmailSend::from_env()?;

        // Built at startup rather than per send, so a missing key is a server
        // that does not start rather than one that fails the first time
        // somebody tries to sign in.
        let resend = match email_send {
            EmailSend::Resend => Some(ResendClient::new(&ResendCredentials::from_env()?)?),
            EmailSend::Log => None,
        };

        if email_send == EmailSend::Log {
            tracing::warn!(
                "EMAIL_SEND=log: sign-in codes are written to this log and no email is sent",
            );
        }

        tracing::info!(
            env = credentials.env.as_str(),
            collect = collect.as_str(),
            email = email_send.as_str(),
            "serving live plaid data",
        );

        let runtime = Arc::new(Self {
            state: RwLock::new(StateServer::new()),
            credentials,
            email_send,
            resend,
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
            .dispatch(vec![
                Event::FromProcess {
                    event: EventProcess::Seeded { seed: fresh_seed() },
                },
                Event::FromProcess {
                    event: EventProcess::Started,
                },
            ])
            .await;

        // Reseeded on a timer, which is what bounds the damage of a leaked
        // log: a seed tells you the codes drawn before the next one and no
        // more. The task holds a weak reference so it cannot keep the server
        // alive past its own shutdown.
        let reseed = Arc::downgrade(&runtime);

        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(RESEED_EVERY);

            // The first tick is immediate and we have just seeded.
            ticks.tick().await;

            loop {
                ticks.tick().await;

                let Some(runtime) = reseed.upgrade() else {
                    return;
                };

                runtime
                    .dispatch(vec![Event::FromProcess {
                        event: EventProcess::Seeded { seed: fresh_seed() },
                    }])
                    .await;
            }
        });

        let router = router_server::router(Arc::clone(&runtime));

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
    pub(crate) async fn dispatch(&self, events: Vec<Event>) -> Vec<Action> {
        if events.is_empty() {
            return Vec::new();
        }

        let now = unix_now();
        let mut actions = Vec::new();

        {
            let mut state = self.write_state();
            state.handle_events(now, &events, &mut actions);
        }

        tracing::debug!(?events, ?actions, "handled");

        // What performing them caused, joined onto what caused it. A handler
        // reads its outcome off this, and the outcome of a link is three
        // dispatches deep — poll, exchange, linked — so a turn that returned
        // only its own actions would report every sign-in as a failure.
        let caused = self.perform_actions(&actions).await;

        actions.into_iter().chain(caused).collect()
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<Action>> + Send + 'a>> {
        Box::pin(async move {
            let mut caused = Vec::new();

            for action in actions {
                caused.extend(self.perform_action(action).await);
            }

            caused
        })
    }

    async fn perform_action(&self, action: &Action) -> Vec<Action> {
        match action {
            Action::ToSource { action } => return self.perform_action_source(action).await,
            Action::ToStore { action } => self.perform_action_store(action).await,
            Action::ToProcess { action } => self.perform_action_process(action),
            Action::ToEmail { action } => self.perform_action_email(action).await,
            // The answer to a request already in hand, so it is the absence
            // of work here rather than a step: the handler reads its outcome
            // off the actions once they are done.
            Action::ToRouter { .. } => {}
        }

        Vec::new()
    }

    /// Get a sign-in code to a person, or say where it went instead.
    async fn perform_action_email(&self, action: &ActionEmail) {
        let ActionEmail::SendCode { email, code } = action;

        match (&self.email_send, &self.resend) {
            (EmailSend::Resend, Some(client)) => {
                let message = src_resend::Email::new(
                    client_from(),
                    email,
                    format!("{code} is your Meridian code"),
                    format!(
                        "Your Meridian sign-in code is {code}.\n\n\
                         It works for the next 10 minutes. If you did not ask for it, \
                         you can ignore this — nobody can sign in without it.",
                    ),
                );

                if let Err(e) = src_resend::send(client, &message).await {
                    tracing::error!(error = %e, "could not send a sign-in code");
                }
            }

            // The code goes in the log, at WARN, saying no email was sent.
            // Loud on purpose: a server accidentally running this way says so
            // on every single sign-in rather than looking healthy.
            _ => tracing::warn!(
                email,
                code,
                "EMAIL_SEND=log: no email sent, the code is on this line",
            ),
        }
    }

    /// Each of these ends by feeding what came back in as the next event, so
    /// they answer with the actions that produced — which is how a handler
    /// several dispatches away learns what happened.
    async fn perform_action_source(&self, action: &ActionSource) -> Vec<Action> {
        match action {
            ActionSource::FetchPlaid { item_id, cursor } => {
                self.fetch_plaid(item_id, cursor.clone()).await
            }

            ActionSource::OpenLink { user_id } => self.open_link(*user_id).await,

            ActionSource::PollLinks { link_tokens } => self.poll_links(link_tokens).await,

            ActionSource::ExchangePublicToken {
                link_token,
                public_token,
            } => self.exchange_public_token(link_token, public_token).await,
        }
    }

    /// Open a Link session and report the page that hosts it.
    async fn open_link(&self, user_id: Uuid) -> Vec<Action> {
        let event = match runtime_plaid::open_link(&self.credentials, &user_id.to_string()).await {
            Ok((link_token, Some(hosted_link_url))) => EventSource::LinkOpened {
                user_id,
                link_token,
                hosted_link_url,
            },

            // A link token with no hosted URL means the request did not ask
            // for Hosted Link, which it always does. Reported rather than
            // guessed at: there is no URL to send anybody to.
            Ok((link_token, None)) => {
                tracing::error!("plaid answered a link token with no hosted url");

                EventSource::LinkAbandoned { link_token }
            }

            Err(e) => {
                tracing::warn!(error = %e, "could not open a link session");

                EventSource::LinkAbandoned {
                    link_token: String::new(),
                }
            }
        };

        self.dispatch(vec![Event::FromSource { event }]).await
    }

    /// Ask what became of every Link session the machine named.
    ///
    /// One call each, and one event for all of them. Which sessions to ask
    /// about is the machine's decision; how many requests that takes is not.
    async fn poll_links(&self, link_tokens: &[String]) -> Vec<Action> {
        let mut polls = Vec::with_capacity(link_tokens.len());

        for link_token in link_tokens {
            let poll = match runtime_plaid::poll_link(&self.credentials, link_token).await {
                Ok((finished, public_tokens)) => crate::LinkPoll {
                    link_token: link_token.clone(),
                    finished,
                    public_tokens,
                },

                Err(e) => {
                    tracing::warn!(error = %e, "could not poll a link session");

                    // Not abandoned: a failed poll says nothing about the
                    // session, and giving up here would lose a bank somebody
                    // did connect.
                    crate::LinkPoll {
                        link_token: link_token.clone(),
                        finished: false,
                        public_tokens: Vec::new(),
                    }
                }
            };

            polls.push(poll);
        }

        self.dispatch(vec![Event::FromSource {
            event: EventSource::LinksPolled { polls },
        }])
        .await
    }

    /// Trade a `public_token` for the permanent one.
    ///
    /// **The one call in this server whose effect cannot be undone.** In
    /// Production it spends an Item slot and `/item/remove` does not give it
    /// back; `fetch_all`'s Sandbox guard is what keeps that unreachable today.
    ///
    /// The access token goes into the runtime's map and no further. It never
    /// becomes an event, because an event carrying one would put a bank
    /// credential in the log.
    async fn exchange_public_token(&self, link_token: &str, public_token: &str) -> Vec<Action> {
        let event = match runtime_plaid::exchange(&self.credentials, public_token).await {
            Ok((item_id, access_token)) => {
                self.write_tokens().insert(item_id.clone(), access_token);

                EventSource::ItemLinked {
                    link_token: link_token.to_string(),
                    item_id,
                }
            }

            Err(e) => {
                tracing::warn!(error = %e, "could not exchange a public token");

                EventSource::LinkAbandoned {
                    link_token: link_token.to_string(),
                }
            }
        };

        self.dispatch(vec![Event::FromSource { event }]).await
    }

    /// Everything the store is asked for.
    ///
    /// Each arm ends by feeding the outcome back in as an event, which is what
    /// closes the loop: nothing here inspects an answer or decides anything
    /// about it.
    async fn perform_action_store(&self, action: &ActionStore) {
        let db = &self.db;

        match action {
            ActionStore::SaveCode {
                email,
                code_sha256,
                expires_at,
            } => {
                if let Err(e) = src_postgres::save_code(db, email, code_sha256, *expires_at).await {
                    self.report_save_failed(e).await;
                }
            }

            ActionStore::RecordAttempt { email } => {
                if let Err(e) = src_postgres::record_attempt(db, email).await {
                    self.report_save_failed(e).await;
                }
            }

            ActionStore::DeleteCode { email } => {
                if let Err(e) = src_postgres::delete_code(db, email).await {
                    self.report_save_failed(e).await;
                }
            }

            ActionStore::CreateSession {
                email,
                token_sha256,
                expires_at,
            } => {
                // An address becomes a user here, which is what makes there be
                // no separate sign-up: one that exists signs in, one that does
                // not signs up, so there is nothing to choose and no way to
                // choose wrong. A lookup, not a decision — the machine has no
                // use for a uuid.
                let created = async {
                    let user_id = src_postgres::upsert_user(db, email).await?;

                    src_postgres::create_session(db, token_sha256, user_id, Some(*expires_at))
                        .await?;

                    tracing::info!(%user_id, "signed in");

                    Ok::<(), src_postgres::PostgresError>(())
                };

                if let Err(e) = created.await {
                    self.report_save_failed(e).await;
                }
            }

            ActionStore::SaveLink {
                link_token,
                user_id,
            } => {
                if let Err(e) = src_postgres::save_link(db, link_token, *user_id).await {
                    self.report_save_failed(e).await;
                }
            }

            ActionStore::CollectLink { link_token } => {
                if let Err(e) = src_postgres::collect_link(db, link_token).await {
                    self.report_save_failed(e).await;
                }
            }

            ActionStore::SaveItem {
                item_id,
                link_token,
            } => {
                if let Err(e) = self.save_linked_item(db, item_id, link_token).await {
                    self.report_save_failed(e).await;
                }
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

    /// Write a newly linked bank.
    ///
    /// The owner comes from the Link session rather than from the action: the
    /// session was written down against whoever asked for it, before its URL
    /// was ever handed over, which is what stops one account collecting a bank
    /// another account connected.
    async fn save_linked_item(
        &self,
        db: &Db,
        item_id: &str,
        link_token: &str,
    ) -> Result<(), src_postgres::PostgresError> {
        let Some(access_token) = self.token_for(item_id) else {
            tracing::error!(item_id, "a bank was linked with no token held for it");

            return Ok(());
        };

        let Some(user_id) = src_postgres::user_for_link(db, link_token).await? else {
            tracing::error!(link_token, "a bank was linked against no session");

            return Ok(());
        };

        src_postgres::save_item(
            db,
            &src_postgres::StoredItem {
                item_id: item_id.to_string(),
                user_id,
                access_token,
                institution_id: None,
                institution_name: None,
                cursor: None,
                error_code: None,
                synced_at: Timestamp::EPOCH,
            },
        )
        .await?;

        tracing::info!(item_id, %user_id, "bank linked");

        Ok(())
    }

    /// A failed write, as the event that says so. The machine logs it and
    /// carries on; what it must not do is find out by a silence.
    async fn report_save_failed(&self, e: src_postgres::PostgresError) {
        self.dispatch(vec![Event::FromStore {
            event: EventStore::SaveFailed {
                message: e.to_string(),
            },
        }])
        .await;
    }

    /// One user's banks, as the store holds them.
    ///
    /// Read on the request path, for the person who asked. That is the change
    /// that let this server serve more than one of them: the alternative was
    /// every user's ledger resident in one process, read wholesale at boot.
    ///
    /// The connections are assembled here rather than in the store, because a
    /// connection is a *projection* — initials, a worded status — and
    /// `src-postgres` returns records.
    async fn connections_for(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<api_ledger::Connection>, src_postgres::PostgresError> {
        let items = src_postgres::load_items(&self.db, user_id).await?;

        let mut connections = Vec::with_capacity(items.len());

        for item in &items {
            let accounts = src_postgres::load_accounts(&self.db, &item.item_id).await?;

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

        Ok(connections)
    }

    /// The sign-in code standing against an address, if there is one.
    ///
    /// Read on the request path and handed to the machine on the event, the
    /// same as every other fact: the machine keeps no codes, so it cannot
    /// decide about one it was not given.
    pub(crate) async fn stored_code(
        &self,
        email: &str,
    ) -> Result<Option<StoredCode>, src_postgres::PostgresError> {
        Ok(src_postgres::load_code(&self.db, email)
            .await?
            .map(|s| StoredCode {
                code_sha256: s.code_sha256,
                expires_at: s.expires_at,
                attempts: s.attempts,
            }))
    }

    /// The Link sessions this user has open and has not collected.
    pub(crate) async fn pending_links(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<String>, src_postgres::PostgresError> {
        Ok(src_postgres::pending_links(&self.db, user_id)
            .await?
            .into_iter()
            .map(|link| link.link_token)
            .collect())
    }

    /// Whether the store answers. Asked rather than assumed from a pool.
    pub(crate) async fn ping_store(&self) -> Result<(), src_postgres::PostgresError> {
        src_postgres::ping(&self.db).await
    }

    /// The facts the machine needs to decide whether to sync each of them.
    pub(crate) async fn facts_for(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<crate::ItemFacts>, src_postgres::PostgresError> {
        let items = src_postgres::load_items(&self.db, user_id).await?;

        // Every token this user's banks need, so the fetch below can find one
        // without going back to the database for it.
        {
            let mut tokens = self.write_tokens();

            for item in &items {
                tokens.insert(item.item_id.clone(), item.access_token.clone());
            }
        }

        Ok(items
            .iter()
            .map(|item| crate::ItemFacts {
                item_id: item.item_id.clone(),
                cursor: item.cursor.clone(),
                synced_at: Some(item.synced_at),
                login_required: item.error_code.as_deref() == Some("ITEM_LOGIN_REQUIRED"),
            })
            .collect())
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

                // The Item row already exists — a sync can only happen for a
                // bank that was linked — so this is the institution name
                // catching up, and `user_id` is deliberately not touched.
                src_postgres::name_item(db, item_id, institution_name.as_deref()).await?;

                let _ = access_token;

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

            // Carried out where they are decided, above. `write` is the sync
            // path, whose ordering is load-bearing in a way these are not.
            ActionStore::SaveCode { .. }
            | ActionStore::RecordAttempt { .. }
            | ActionStore::DeleteCode { .. }
            | ActionStore::CreateSession { .. }
            | ActionStore::SaveLink { .. }
            | ActionStore::CollectLink { .. }
            | ActionStore::SaveItem { .. } => {}
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
    async fn fetch_plaid(&self, item_id: &str, cursor: Option<String>) -> Vec<Action> {
        // One fetch at a time. A burst of cold requests would otherwise all
        // miss and all call out.
        let Ok(_guard) = self.fetching.try_lock() else {
            tracing::debug!("a fetch is already in flight");
            return Vec::new();
        };

        // The machine named an Item it read off its own connection list, and
        // every connection there was built from a stored Item. Absent means
        // those two have drifted, which is worth saying rather than guessing
        // at.
        let Some(access_token) = self.token_for(item_id) else {
            tracing::error!(item_id, "asked to fetch an item with no token");

            return self
                .dispatch(vec![Event::FromSource {
                    event: EventSource::PlaidFailed {
                        item_id: item_id.to_string(),
                        code: String::from("NO_ACCESS_TOKEN"),
                    },
                }])
                .await;
        };

        let event =
            match runtime_plaid::fetch_all(&self.credentials, &access_token, cursor, unix_now())
                .await
            {
                Ok(fetched) => EventSource::PlaidLoaded {
                    item_id: item_id.to_string(),
                    added: fetched.added,
                    modified: fetched.modified,
                    removed: fetched.removed,
                    connection: fetched.connections.into_iter().next(),
                    streams: fetched.streams,
                    cursor: fetched.cursor,
                },

                Err(e) => {
                    let code = match &e {
                        src_plaid::PlaidError::Plaid { error_code, .. } => error_code.clone(),
                        _ => "REQUEST_FAILED".to_string(),
                    };

                    tracing::warn!(error = %e, code, "plaid fetch failed");

                    EventSource::PlaidFailed {
                        item_id: item_id.to_string(),
                        code,
                    }
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

        let caused = self.perform_actions(&actions).await;

        actions.into_iter().chain(caused).collect()
    }

    /// Everything the app knows, as of now.
    ///
    /// The guard is taken and dropped inside this function rather than held by
    /// the caller: an `RwLockReadGuard` alive across an `.await` makes the
    /// handler's future `!Send`, which axum will not accept.
    /// Everything one user knows, as of now, read from the store.
    ///
    /// Not from memory. The machine used to hold the ledger and this used to
    /// be a projection of it, which was only ever right because there was one
    /// user — and a second would have been served the first one's transactions.
    pub(crate) async fn project_snapshot(&self, user_id: Uuid) -> WireSnapshot {
        let now = unix_now();

        let entries = src_postgres::load_entries(&self.db, user_id).await;
        let streams = src_postgres::load_streams(&self.db, user_id).await;
        let connections = self.connections_for(user_id).await;

        // A store that will not answer is reported and served around: an empty
        // ledger is what a first run has, and it is a better answer than a
        // failure the phone would have to have a screen for.
        if let Err(e) = entries.as_ref().err().map_or(Ok(()), Err) {
            tracing::error!(error = %e, "could not read the ledger");
        }

        WireSnapshot::new(
            &entries.unwrap_or_default(),
            &connections.unwrap_or_default(),
            &streams.unwrap_or_default(),
            now,
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

        tracing::info!("shutting down");
    }

    /// Who this request is, or 401.
    ///
    /// The only place a request turns into a user. Everything downstream takes
    /// a `Uuid` rather than a header, so there is no path that reads somebody's
    /// ledger without having come through here.
    pub(crate) async fn authenticate(
        &self,
        headers: &axum::http::HeaderMap,
    ) -> Result<Uuid, StatusCode> {
        let header = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());

        let token = session_token::from_header(header).ok_or(StatusCode::UNAUTHORIZED)?;

        let found =
            src_postgres::user_for_session(&self.db, &session_token::hash(token), unix_now())
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "could not read the session");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

        found.ok_or(StatusCode::UNAUTHORIZED)
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

/// 32 bytes from the operating system.
///
/// The one place entropy enters, and it is at the edge — the machine has no
/// source of its own, the same way it has no clock. `rand::rng()` is the OS
/// CSPRNG, which is the only acceptable origin for a seed that credentials are
/// drawn from.
fn fresh_seed() -> [u8; 32] {
    rand::random()
}

/// Who the sign-in email comes from. Read once per send rather than held,
/// because it is only consulted on the Resend path.
fn client_from() -> String {
    std::env::var("RESEND_FROM").unwrap_or_else(|_| String::from("meridian@example.com"))
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
