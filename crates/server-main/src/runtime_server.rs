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
use tokio::sync::Notify;

use crate::Action;
use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionRouter;
use crate::ActionSource;
use crate::ActionStore;
use crate::Client;
use crate::EmailSend;
use crate::Event;
use crate::EventProcess;
use crate::EventRouter;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;
use crate::PlaidCollect;
use crate::PublicUrl;
use crate::StateServer;
use crate::StoredCode;
use crate::WebOrigins;
use crate::WebhookFacts;
use crate::project;
use crate::router_exchange::RouterExchange;
use crate::router_server;
use crate::runtime_plaid;
use crate::source_pool::SourcePool;
use crate::webhook_verifier::WebhookVerifier;
use server_wire::WireSnapshot;
use src_plaid::PlaidCredentials;
use src_plaid::PlaidRejection;
use src_plaid::PlaidWebhook;
use src_plaid::PlaidWebhookKind;
use src_postgres::Db;
use src_postgres::DbKey;
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

/// How often the machine is handed everything that has arrived since.
///
/// The sleep is what decides how much a batch collects, and it costs one of
/// itself per turn — a link takes three, so this multiplies. Small enough that
/// three of them are lost in the noise of one Plaid round trip, which is what
/// a turn is really waiting on.
const TURN_EVERY: std::time::Duration = std::time::Duration::from_millis(2);

/// How many calls to Plaid may be in flight at once.
///
/// More than one, because somebody with three banks should not sync them one
/// after another. Not many more: Plaid rate-limits per Item and the machine is
/// what respects that, so this only bounds how much work is outstanding.
const PLAID_AT_ONCE: usize = 4;

/// How many sign-in emails may be in flight at once. Small: it is one request
/// per person signing in, and they are not waiting on it.
const EMAIL_AT_ONCE: usize = 2;

/// How often the machine is told what time it is.
///
/// A second, which is far finer than anything the machine measures — the
/// shortest window in it is thirty seconds — and fine enough that giving up on
/// a call is not noticeably late.
const TICK_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// How many writes may be in flight at once. Bounded by what the Postgres
/// pool will hand out either way; this is what stops a burst queueing there
/// instead, where it is invisible.
const STORE_AT_ONCE: usize = 8;

pub struct RuntimeServer {
    /// The store. Required, and not optional in any sense: the access tokens
    /// live in it, so a server without one has no way to reach any bank and
    /// nothing to serve. It used to be optional and hold state in memory,
    /// which was the last variable in the server with a fallback behind it.
    db: Db,

    /// What the access token column is sealed with.
    ///
    /// Beside the pool because it is the same kind of fact — how to reach the
    /// store, and how to read the one column in it that is not plain. Read on
    /// the way up with everything else, so a key that is missing or the wrong
    /// length is a server that does not start.
    db_key: DbKey,

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
    /// Only the loop takes it now, once per turn, for the length of a pure
    /// synchronous call. Nothing else touches the machine, and no guard ever
    /// spans an `.await` — the calls to Plaid happen in a pool that has no
    /// reference to it at all.
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

    /// Where Plaid should call, or `None` when nothing should.
    ///
    /// `Some` exactly when `PLAID_COLLECT=webhook`, because that is the only
    /// mode in which an address means anything — and in that mode it is
    /// required, read and validated on the way up. A server that cannot name
    /// itself does not start, rather than starting and telling Plaid nothing.
    webhook: Option<String>,

    /// Whether a POST on the webhook route is really Plaid, and the signing
    /// keys that answer it.
    ///
    /// Here rather than in the machine for the same reason the access tokens
    /// are: it holds keys and it does IO, and the machine's job is to be
    /// logged and replayed. What comes out of it is a fact — this webhook is
    /// genuine and says the following — and every decision downstream of that
    /// is the machine's.
    webhooks: WebhookVerifier,

    /// Raised once the machine asks to shut down.
    shutdown: Notify,

    /// Where a route's question is paired with its answer. Shared, because
    /// the pools push what a call produced straight onto it.
    exchange: Arc<RouterExchange>,

    /// Every call to Plaid. Two at once: a snapshot of somebody with several
    /// banks should overlap, and Plaid's own limits are the machine's problem
    /// rather than this number's.
    plaid: SourcePool,

    /// Every sign-in email. Its own pool so a slow bank cannot use up the room
    /// a sign-in needs — the whole reason there is one per source.
    email: SourcePool,

    /// Every write. The store is a source like any other now: it is asked to
    /// do something and reports back that it did, and the machine holds the
    /// route's answer until it has.
    store: SourcePool,
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

        // Required in one mode and meaningless in the other, the same shape as
        // `RESEND_*` under `EMAIL_SEND`. Read here rather than when a link is
        // opened, so a server that cannot name its own address says so in its
        // first second instead of on the day somebody connects a bank.
        let public_url = match collect {
            PlaidCollect::Webhook => Some(PublicUrl::from_env()?),
            PlaidCollect::Poll => None,
        };

        let db = Self::open_store().await?;

        // Before the first query rather than at the first bank connection: a
        // server that cannot open the column it keeps credentials in has
        // nothing to serve, and the honest moment to say so is now.
        let db_key = DbKey::from_env()?;

        let email_send = EmailSend::from_env()?;

        // Read on the way up with the rest, though nothing serves a browser
        // until the first request. A bad value here is the one misconfiguration
        // this process can never notice on its own: a browser refused by CORS
        // never sends the request, so the failure looks like silence from the
        // server and like a broken web app from the outside.
        let origins = WebOrigins::from_env()?;

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

        // The address Plaid is told to call, worked out once and used for both
        // the log and the client. `PUBLIC_URL` is the base; this is the route
        // on the end of it, and it is what the log has to name — the base
        // alone is neither the address we advertise nor the one we answer on.
        let webhook = public_url.as_ref().map(PublicUrl::webhook);

        tracing::info!(
            env = credentials.env.as_str(),
            collect = collect.as_str(),
            webhook = webhook.as_deref().unwrap_or("none"),
            email = email_send.as_str(),
            origins = origins.as_str(),
            "serving live plaid data",
        );

        let exchange = Arc::new(RouterExchange::default());

        let runtime = Arc::new(Self {
            plaid: SourcePool::new("plaid", PLAID_AT_ONCE, Arc::clone(&exchange)),
            email: SourcePool::new("email", EMAIL_AT_ONCE, Arc::clone(&exchange)),
            store: SourcePool::new("store", STORE_AT_ONCE, Arc::clone(&exchange)),
            exchange,
            state: RwLock::new(StateServer::new()),
            credentials,
            email_send,
            resend,
            tokens: RwLock::new(HashMap::new()),
            webhooks: WebhookVerifier::default(),
            webhook,
            db,
            db_key,
            shutdown: Notify::new(),
        });

        // The first event, and the reason the boot sequence is replayable: the
        // machine asks to be filled from the store rather than the runtime
        // deciding to do it on its own initiative.
        // The clock first. The machine starts at the epoch, and a code minted
        // before the first tick would expire in 1970.
        runtime.exchange.push(Event::FromProcess {
            event: EventProcess::Tick { now: unix_now() },
        });

        runtime.exchange.push(Event::FromProcess {
            event: EventProcess::Started { collect },
        });

        runtime.exchange.push(Event::FromProcess {
            event: EventProcess::Seeded { seed: fresh_seed() },
        });

        // The clock, from the edge, on its own timer. Nothing inside the
        // machine reads one — this is the only place it enters — and it is
        // also the only event that arrives when nothing has happened, which
        // is what lets a call that went quiet be given up on.
        let ticking = Arc::downgrade(&runtime);

        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(TICK_EVERY);

            loop {
                ticks.tick().await;

                let Some(runtime) = ticking.upgrade() else {
                    return;
                };

                runtime.exchange.push(Event::FromProcess {
                    event: EventProcess::Tick { now: unix_now() },
                });
            }
        });

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

                runtime.exchange.push(Event::FromProcess {
                    event: EventProcess::Seeded { seed: fresh_seed() },
                });
            }
        });

        // The loop, and the only caller of `handle_events`. Polling rather
        // than waking on a push: this is the whole implementation, and a few
        // milliseconds is invisible next to the Plaid call a turn is usually
        // waiting on. A longer sleep only means more events land in one batch.
        //
        // Weak, like the reseed task, so it cannot keep the server alive past
        // its own shutdown.
        let turning = Arc::downgrade(&runtime);

        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(TURN_EVERY);

            loop {
                ticks.tick().await;

                let Some(runtime) = turning.upgrade() else {
                    return;
                };

                runtime.turn();
            }
        });

        let router = router_server::router(Arc::clone(&runtime), &origins);

        // Every interface rather than loopback, because a phone on the LAN is
        // one of the clients. The simulator shares the Mac's loopback and would
        // be happy either way; a real device has to reach the Mac by its
        // address on the network, and nothing answers there if we bind
        // loopback. This is a development server on a home network.
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", PORT)).await?;

        tracing::info!(port = PORT, "server listening");

        let signals = Arc::clone(&runtime);

        // With connection info, so every line about a request can say which
        // client made it. A dev server is talked to by the web front-end on
        // loopback and a phone on the LAN at the same time, and untangling
        // their requests is most of what the log is for.
        let service = router.into_make_service_with_connect_info::<std::net::SocketAddr>();

        axum::serve(listener, service)
            .with_graceful_shutdown(async move { signals.await_shutdown().await })
            .await?;

        tracing::info!("server stopped");

        Ok(())
    }

    /// Ask the machine something on a route's behalf, and wait for its answer.
    ///
    /// The one way a handler reaches the machine.
    pub(crate) async fn request(&self, client: Client, event: EventRouter) -> Option<ActionRouter> {
        self.exchange.request(client, event).await
    }

    /// One turn: everything queued since the last one, handed to the machine
    /// together, and the actions it decided on carried out.
    ///
    /// This is the loop `docs/ARCHITECTURE.md` describes, and it is the only
    /// caller of `handle_events`. A batch is whatever arrived while the last
    /// turn was running, so the machine sees three banks reporting as one
    /// batch rather than three — which is what makes it worth batching at all.
    fn turn(self: &Arc<Self>) {
        let events = self.exchange.drain();

        if events.is_empty() {
            return;
        }

        let mut actions = Vec::new();

        // The guard is held across `handle_events` and nothing else. It is
        // pure and synchronous, so this is microseconds and never spans an
        // `.await` — the IO below happens with the lock long since dropped.
        {
            let mut state = self.write_state();
            state.handle_events(&events, &mut actions);
        }

        tracing::debug!(?events, ?actions, "turn");

        for action in actions {
            self.begin(action);
        }
    }

    /// Hand one action to a pool, to be carried out whenever there is room.
    fn calling(self: &Arc<Self>, pool: &SourcePool, action: Action) {
        let runtime = Arc::clone(self);

        pool.call(async move { runtime.perform_action(&action).await });
    }

    /// Set one action going.
    ///
    /// **Nothing here waits.** Every call goes to the pool for its source and
    /// comes back as an event whenever it lands, which is what keeps one slow
    /// bank out of every other route's turns. Which pool is the only thing
    /// decided: a bank, an email and the store are three sources, so a slow
    /// one cannot crowd out the others.
    fn begin(self: &Arc<Self>, action: Action) {
        match action {
            // The answer somebody is parked on. There is no IO in handing it
            // over, and by now everything it was waiting on has landed — the
            // machine held it until the last write reported.
            Action::ToRouter { request_id, action } => {
                self.exchange.deliver(request_id, action);
            }

            Action::ToSource { .. } => self.calling(&self.plaid, action),

            Action::ToEmail { .. } => self.calling(&self.email, action),

            Action::ToStore { .. } => self.calling(&self.store, action),

            // No IO and nothing to wait for: a notify and a log line.
            Action::ToProcess { action } => self.perform_action_process(&action),
        }
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

    /// Carry one action out, and report what came of it.
    ///
    /// Whatever it produces is an *event*, never another action: deciding what
    /// to do about an answer is the machine's job, and returning actions from
    /// here would be this function deciding instead. That is also what
    /// flattened the loop — there is no recursion left to bound.
    async fn perform_action(&self, action: &Action) -> Vec<Event> {
        match action {
            Action::ToSource { action } => return self.perform_action_source(action).await,
            Action::ToStore { action } => return self.perform_action_store(action).await,
            Action::ToEmail { action } => self.perform_action_email(action).await,
            // Both are handled where the action is set going: an answer goes
            // to the route parked on it, and a process action is a notify.
            Action::ToRouter { .. } | Action::ToProcess { .. } => {
                unreachable!("delivered rather than performed")
            }
        }

        Vec::new()
    }

    /// Get a sign-in code to a person, or say where it went instead.
    async fn perform_action_email(&self, action: &ActionEmail) {
        let ActionEmail::SendCode { email, code } = action;

        match (&self.email_send, &self.resend) {
            (EmailSend::Resend, Some(client)) => {
                let message = src_resend::Email::new(
                    client.from(),
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
    async fn perform_action_source(&self, action: &ActionSource) -> Vec<Event> {
        match action {
            ActionSource::FetchPlaid {
                call_id,
                item_id,
                cursor,
            } => self.fetch_plaid(*call_id, item_id, cursor.clone()).await,

            ActionSource::OpenLink { call_id, user_id } => self.open_link(*call_id, *user_id).await,

            ActionSource::PollLinks {
                call_id,
                link_tokens,
            } => self.poll_links(*call_id, link_tokens).await,

            ActionSource::ExchangePublicToken {
                call_id,
                link_token,
                public_token,
            } => {
                self.exchange_public_token(*call_id, link_token, public_token)
                    .await
            }
        }
    }

    /// Open a Link session and report the page that hosts it.
    async fn open_link(&self, call_id: crate::CallId, user_id: Uuid) -> Vec<Event> {
        let event = match runtime_plaid::open_link(
            &self.credentials,
            &user_id.to_string(),
            self.webhook.as_deref(),
        )
        .await
        {
            Ok((link_token, Some(hosted_link_url))) => EventSource::LinkOpened {
                call_id,
                user_id,
                link_token,
                hosted_link_url,
            },

            // A link token with no hosted URL means the request did not ask
            // for Hosted Link, which it always does. Reported rather than
            // guessed at: there is no URL to send anybody to.
            Ok((link_token, None)) => {
                tracing::error!("plaid answered a link token with no hosted url");

                EventSource::LinkAbandoned {
                    call_id,
                    link_token,
                }
            }

            Err(e) => {
                tracing::warn!(error = %e, "could not open a link session");

                EventSource::LinkAbandoned {
                    call_id,
                    link_token: String::new(),
                }
            }
        };

        vec![Event::FromSource { event }]
    }

    /// Ask what became of every Link session the machine named.
    ///
    /// One call each, and one event for all of them. Which sessions to ask
    /// about is the machine's decision; how many requests that takes is not.
    async fn poll_links(&self, call_id: crate::CallId, link_tokens: &[String]) -> Vec<Event> {
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

        vec![Event::FromSource {
            event: EventSource::LinksPolled { call_id, polls },
        }]
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
    async fn exchange_public_token(
        &self,
        call_id: crate::CallId,
        link_token: &str,
        public_token: &str,
    ) -> Vec<Event> {
        let event = match runtime_plaid::exchange(&self.credentials, public_token).await {
            Ok((item_id, access_token)) => {
                self.write_tokens().insert(item_id.clone(), access_token);

                EventSource::ItemLinked {
                    call_id,
                    link_token: link_token.to_string(),
                    item_id,
                }
            }

            Err(e) => {
                tracing::warn!(error = %e, "could not exchange a public token");

                EventSource::LinkAbandoned {
                    call_id,
                    link_token: link_token.to_string(),
                }
            }
        };

        vec![Event::FromSource { event }]
    }

    /// Everything the store is asked for.
    ///
    /// Whether it landed, as the event that says so.
    ///
    /// One report for every write, against the call that asked for it. The
    /// machine holds a route's answer until this arrives, so a write that
    /// reported nothing would be a request that never returns — which is why
    /// the failure is an event too, rather than a line in a log.
    async fn perform_action_store(&self, action: &ActionStore) -> Vec<Event> {
        // A read answers with what it found; a write answers that it landed.
        // Two shapes, so they are told apart here rather than inside
        // `write_store`, which has nothing to report but success.
        let event = match action {
            ActionStore::LoadSession {
                call_id,
                token_sha256,
                now,
            } => self.load_session(*call_id, token_sha256, *now).await,

            ActionStore::LoadItems { call_id, user_id } => {
                self.load_items(*call_id, *user_id).await
            }

            ActionStore::LoadPendingLinks { call_id, user_id } => {
                self.load_pending_links(*call_id, *user_id).await
            }

            ActionStore::LoadSnapshot { call_id, user_id } => EventStore::SnapshotLoaded {
                call_id: *call_id,
                snapshot: self.project_snapshot(*user_id).await,
            },

            ActionStore::LoadCode { call_id, email } => self.load_code(*call_id, email).await,

            _ => {
                let call_id = action.call_id();

                match self.write_store(action).await {
                    Ok(()) => EventStore::WriteSuccess { call_id },

                    Err(e) => EventStore::WriteError {
                        call_id,
                        message: e.to_string(),
                    },
                }
            }
        };

        vec![Event::FromStore { event }]
    }

    /// Who a session token signs in, as the store holds it.
    ///
    /// The clock comes from the action rather than from here: the row this
    /// matches is the one the machine reckoned was still good.
    async fn load_session(
        &self,
        call_id: crate::CallId,
        token_sha256: &[u8],
        now: Timestamp,
    ) -> EventStore {
        match src_postgres::user_for_session(&self.db, token_sha256, now).await {
            Ok(user_id) => EventStore::SessionResolved { call_id, user_id },

            Err(e) => EventStore::ReadError {
                call_id,
                message: e.to_string(),
            },
        }
    }

    /// Which banks are this user's, and where each one's last sync reached.
    async fn load_items(&self, call_id: crate::CallId, user_id: Uuid) -> EventStore {
        match self.facts_for(user_id).await {
            Ok(items) => EventStore::ItemsLoaded { call_id, items },

            Err(e) => EventStore::ReadError {
                call_id,
                message: e.to_string(),
            },
        }
    }

    /// The Link sessions this user has open and has not collected.
    async fn load_pending_links(&self, call_id: crate::CallId, user_id: Uuid) -> EventStore {
        match self.pending_links(user_id).await {
            Ok(link_tokens) => EventStore::PendingLinksLoaded {
                call_id,
                link_tokens,
            },

            Err(e) => EventStore::ReadError {
                call_id,
                message: e.to_string(),
            },
        }
    }

    /// What code stands against an address, as the store holds it.
    async fn load_code(&self, call_id: crate::CallId, email: &str) -> EventStore {
        match src_postgres::load_code(&self.db, email).await {
            Ok(stored) => EventStore::CodeLoaded {
                call_id,
                stored: stored.map(|s| StoredCode {
                    code_sha256: s.code_sha256,
                    expires_at: s.expires_at,
                    attempts: s.attempts,
                }),
            },

            Err(e) => EventStore::ReadError {
                call_id,
                message: e.to_string(),
            },
        }
    }

    /// The write itself. Nothing here inspects an answer or decides anything
    /// about it; `perform_action_store` turns the outcome into the event.
    async fn write_store(&self, action: &ActionStore) -> Result<(), src_postgres::PostgresError> {
        let db = &self.db;

        match action {
            ActionStore::SaveCode {
                call_id: _,
                email,
                code_sha256,
                expires_at,
            } => {
                src_postgres::save_code(db, email, code_sha256, *expires_at).await?;
            }

            ActionStore::RecordAttempt { call_id: _, email } => {
                src_postgres::record_attempt(db, email).await?;
            }

            ActionStore::DeleteCode { call_id: _, email } => {
                src_postgres::delete_code(db, email).await?;
            }

            ActionStore::CreateSession {
                call_id: _,
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

                created.await?;
            }

            ActionStore::SweepSessions { call_id: _, now } => {
                let gone = src_postgres::sweep_sessions(db, *now).await?;

                // Silent when there was nothing to drop, which is most hours.
                // A line an hour saying so would be the log's most common
                // entry and its least informative.
                if gone > 0 {
                    tracing::info!(sessions = gone, "swept expired sessions");
                }
            }

            ActionStore::SaveLink {
                call_id: _,
                link_token,
                user_id,
            } => {
                src_postgres::save_link(db, link_token, *user_id).await?;
            }

            ActionStore::CollectLink {
                call_id: _,
                link_token,
            } => {
                src_postgres::collect_link(db, link_token).await?;
            }

            ActionStore::SaveItem {
                call_id: _,
                item_id,
                link_token,
            } => {
                self.save_linked_item(db, item_id, link_token).await?;
            }

            ActionStore::SaveFetched { .. } | ActionStore::SaveStatus { .. } => {
                self.write(db, action).await?;
            }

            ActionStore::LoadSession { .. }
            | ActionStore::LoadItems { .. }
            | ActionStore::LoadPendingLinks { .. }
            | ActionStore::LoadSnapshot { .. }
            | ActionStore::LoadCode { .. } => {
                unreachable!("a read is answered by `perform_action_store`")
            }
        }

        Ok(())
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
            &self.db_key,
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
        let items = src_postgres::load_items(&self.db, &self.db_key, user_id).await?;

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

    /// The Link sessions this user has open and has not collected.
    ///
    /// A session is collected by whatever *reports* it, so an open one is one
    /// nobody has been told about — which is exactly what a `SESSION_FINISHED`
    /// is matched against, and what a poll goes and asks about.
    async fn pending_links(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<String>, src_postgres::PostgresError> {
        Ok(src_postgres::pending_links(&self.db, user_id)
            .await?
            .into_iter()
            .map(|link| link.link_token)
            .collect())
    }

    /// Whether a POST on the webhook route is really Plaid, and what it said.
    ///
    /// The twin of [`authenticate`](RuntimeServer::authenticate), and the only
    /// place a webhook becomes something the machine will be told about. The
    /// bytes are the ones that arrived, untouched — see
    /// [`WebhookVerifier::believe`](crate::webhook_verifier::WebhookVerifier).
    pub(crate) async fn believe_webhook(
        &self,
        headers: &axum::http::HeaderMap,
        body: &[u8],
    ) -> Result<PlaidWebhook, PlaidRejection> {
        let webhook = self
            .webhooks
            .believe(&self.credentials, headers, body, unix_now())
            .await?;

        tracing::info!(
            webhook_type = webhook.webhook_type,
            webhook_code = webhook.webhook_code,
            "a webhook arrived from plaid",
        );

        Ok(webhook)
    }

    /// What a webhook names, as the store holds it.
    ///
    /// The read that has to happen before the machine can decide anything: a
    /// webhook arrives with no session and names a bank rather than a person,
    /// so where its last sync reached is looked up here, exactly as
    /// [`facts_for`](RuntimeServer::facts_for) does on the routes that have a
    /// caller to scope on.
    pub(crate) async fn webhook_facts(&self, webhook: PlaidWebhook) -> WebhookFacts {
        match webhook.kind() {
            PlaidWebhookKind::SessionFinished {
                link_token,
                public_tokens,
            } => WebhookFacts::SessionFinished {
                link_token,
                public_tokens,
            },

            PlaidWebhookKind::SyncUpdatesAvailable { item_id } => {
                WebhookFacts::SyncUpdatesAvailable {
                    item: self.item_facts(&item_id).await,
                    item_id,
                }
            }

            PlaidWebhookKind::LoginRequired { item_id } => WebhookFacts::Login {
                item_id,
                required: true,
            },

            PlaidWebhookKind::LoginRepaired { item_id } => WebhookFacts::Login {
                item_id,
                required: false,
            },

            PlaidWebhookKind::Other => WebhookFacts::Ignored {
                webhook_type: webhook.webhook_type,
                webhook_code: webhook.webhook_code,
            },
        }
    }

    /// One bank, by the id Plaid names it with, and its token remembered.
    ///
    /// `None` covers both a bank the store has no row for and a store that
    /// would not answer. The machine is told the same thing either way,
    /// because there is nothing it could do differently: it has no cursor to
    /// sync from.
    async fn item_facts(&self, item_id: &str) -> Option<crate::ItemFacts> {
        let item = match src_postgres::load_item(&self.db, &self.db_key, item_id).await {
            Ok(item) => item?,

            Err(e) => {
                tracing::error!(item_id, error = %e, "could not read the bank a webhook names");

                return None;
            }
        };

        // The same as `facts_for`: the token goes into the runtime's map so a
        // fetch the machine asks for can find one without going back.
        self.write_tokens()
            .insert(item.item_id.clone(), item.access_token.clone());

        Some(crate::ItemFacts {
            item_id: item.item_id,
            cursor: item.cursor,
            synced_at: Some(item.synced_at),
            login_required: item.error_code.as_deref() == Some(src_plaid::ITEM_LOGIN_REQUIRED),
        })
    }

    /// The facts the machine needs to decide whether to sync each of them.
    async fn facts_for(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<crate::ItemFacts>, src_postgres::PostgresError> {
        let items = src_postgres::load_items(&self.db, &self.db_key, user_id).await?;

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
                call_id: _,
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
                call_id: _,
                item_id,
                error_code,
            } => {
                src_postgres::save_error(db, item_id, error_code.as_deref()).await?;
            }

            // Carried out where they are decided, above. `write` is the sync
            // path, whose ordering is load-bearing in a way these are not.
            ActionStore::LoadSession { .. }
            | ActionStore::LoadItems { .. }
            | ActionStore::LoadPendingLinks { .. }
            | ActionStore::LoadSnapshot { .. }
            | ActionStore::LoadCode { .. }
            | ActionStore::SaveCode { .. }
            | ActionStore::RecordAttempt { .. }
            | ActionStore::DeleteCode { .. }
            | ActionStore::CreateSession { .. }
            | ActionStore::SweepSessions { .. }
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
    async fn fetch_plaid(
        &self,
        call_id: crate::CallId,
        item_id: &str,
        cursor: Option<String>,
    ) -> Vec<Event> {
        // The machine named an Item it read off its own connection list, and
        // every connection there was built from a stored Item. Absent means
        // those two have drifted, which is worth saying rather than guessing
        // at.
        let Some(access_token) = self.token_for(item_id) else {
            tracing::error!(item_id, "asked to fetch an item with no token");

            return vec![Event::FromSource {
                event: EventSource::PlaidFailed {
                    call_id,
                    item_id: item_id.to_string(),
                    code: String::from("NO_ACCESS_TOKEN"),
                },
            }];
        };

        // Each stage reports as it lands, straight onto the queue rather than
        // waiting for the return below. Nothing is parked on these — they
        // release no route — so they are facts arriving mid-call, and the
        // machine is what decides they are worth keeping.
        let report = |stage, institution: Option<&str>, accounts, transactions, streams| {
            self.exchange.push(Event::FromSource {
                event: EventSource::PlaidProgressed {
                    item_id: item_id.to_string(),
                    stage,
                    institution: institution.map(str::to_string),
                    accounts,
                    transactions,
                    streams,
                },
            });
        };

        let event = match runtime_plaid::fetch_all(
            &self.credentials,
            &access_token,
            cursor,
            unix_now(),
            report,
        )
        .await
        {
            Ok(fetched) => EventSource::PlaidLoaded {
                call_id,
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
                    call_id,
                    item_id: item_id.to_string(),
                    code,
                }
            }
        };

        vec![Event::FromSource { event }]
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
    async fn project_snapshot(&self, user_id: Uuid) -> WireSnapshot {
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
                self.exchange.push(Event::FromTerminal { event });
            }
            _ = notified => {}
        }

        tracing::info!("shutting down");
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
