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
use tokio::sync::mpsc;

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
use crate::PublicUrl;
use crate::StateServer;
use crate::StoredCode;
use crate::WebOrigins;
use crate::project;
use crate::runtime_plaid;
use crate::runtime_router::ActionRx;
use crate::runtime_router::ActionTx;
use crate::runtime_router::EventRx;
use crate::runtime_router::EventTx;
use crate::runtime_router::RuntimeRouter;
use crate::source_pool::SourcePool;
use server_wire::WireSnapshot;
use src_plaid::PlaidCredentials;
use src_postgres::Db;
use src_postgres::DbKey;
use src_resend::ResendClient;
use src_resend::ResendCredentials;
use uuid::Uuid;

/// How often the machine is handed fresh entropy.
///
/// Every seed is 32 bytes of ChaCha20 state, so one covers far more codes than
/// a minute will ever produce. The interval is not about running out — it is
/// about how much a single leaked seed is worth.
const RESEED_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

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

    /// Where an answer goes. The router is on another task and this is the
    /// only way to reach it.
    answers: ActionTx,

    /// Where a fact that nobody asked for goes: the stages a fetch reports as
    /// it runs. The pools have their own clone of this for what a call
    /// returns; this one is for what it says on the way.
    landed: mpsc::UnboundedSender<Event>,

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

        // Everything a call produces comes back here. Unbounded, deliberately:
        // a pool that had to wait to report would be a pool holding the answer
        // to something the loop is about to be asked about. What bounds the
        // work is each pool's own room, and what bounds the queue is the
        // machine, which will not ask for a call it has no budget for.
        let (landed, mut landing) = mpsc::unbounded_channel::<Event>();

        // The same channel, for the three events that start the machine off.
        let boot = landed.clone();

        // The router, on its own task, reachable only through these two.
        let (answers, action_rx): (ActionTx, ActionRx) = mpsc::unbounded_channel();
        let (event_tx, mut asked): (EventTx, EventRx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            if let Err(e) = RuntimeRouter::run(action_rx, event_tx, origins).await {
                tracing::error!(error = %e, "the router stopped");
            }
        });

        let runtime = Arc::new(Self {
            plaid: SourcePool::new("plaid", PLAID_AT_ONCE, landed.clone()),
            email: SourcePool::new("email", EMAIL_AT_ONCE, landed.clone()),
            store: SourcePool::new("store", STORE_AT_ONCE, landed.clone()),
            answers,
            landed,
            credentials,
            email_send,
            resend,
            tokens: RwLock::new(HashMap::new()),
            webhook,
            db,
            db_key,
        });

        // The machine, owned outright by the loop below. Nothing else has a
        // reference to it, which is what makes `handle_events` an ordinary
        // `&mut` call rather than something taken under a guard.
        let mut state = StateServer::new();

        let mut clock = tokio::time::interval(TICK_EVERY);
        let mut reseed = tokio::time::interval(RESEED_EVERY);

        // Reused every turn. One allocation for the life of the process.
        let mut events: Vec<Event> = Vec::new();
        let mut actions: Vec<Action> = Vec::new();

        // The boot sequence, sent the way everything else arrives rather than
        // pushed in behind the loop's back. The clock is first: the machine
        // starts at the epoch, and a code minted before the first tick would
        // expire in 1970. Then the process saying it is up, and the entropy it
        // needs before it will mint anything.
        for event in [
            EventProcess::Tick { now: unix_now() },
            EventProcess::Started { collect },
            EventProcess::Seeded { seed: fresh_seed() },
        ] {
            let _ = boot.send(Event::FromProcess { event });
        }

        loop {
            tokio::select! {
                _ = clock.tick() => events.push(Event::FromProcess {
                    event: EventProcess::Tick { now: unix_now() },
                }),

                // Reseeded on a timer, which is what bounds the damage of
                // a leaked log: a seed tells you the codes drawn before
                // the next one and no more.
                _ = reseed.tick() => events.push(Event::FromProcess {
                    event: EventProcess::Seeded { seed: fresh_seed() },
                }),

                Some((request_id, client, event)) = asked.recv() => {
                    events.push(Event::FromRouter { request_id, client, event });
                }

                Some(event) = landing.recv() => events.push(event),

                // The signal is not the decision. The machine gets to say
                // what happens next, which is what makes shutdown
                // replayable.
                event = signal_received() => events.push(Event::FromTerminal { event }),
            }

            // Every branch above pushes exactly one event, so this is only
            // reachable if one of them stops doing so.
            if events.is_empty() {
                continue;
            }

            state.handle_events(&events, &mut actions);

            tracing::debug!(?events, ?actions, "turn");

            runtime.perform_actions(&mut actions);

            events.clear();

            // Not `Stopping`: that one is transient, and what is outstanding
            // when a signal arrives still has to come back or be given up on.
            // The machine is what says when there is nothing left, because it
            // is the only thing that knows what it is still owed.
            if state.stopped() {
                break;
            }
        }

        tracing::info!("server stopped");

        Ok(())
    }

    /// Carry out everything the machine decided this turn.
    ///
    /// Drained rather than borrowed, and that is not a copy: every action here
    /// crosses a task boundary — into a pool, or down the channel the router
    /// is on — so it has to be owned either way. Draining moves it; borrowing
    /// would mean cloning it first. The buffer keeps its capacity and is
    /// filled again next turn.
    ///
    /// **Nothing here waits.** Every call goes to the pool for its source and
    /// comes back as an event whenever it lands, which is what keeps one slow
    /// bank out of every other route's turns.
    fn perform_actions(self: &Arc<Self>, actions: &mut Vec<Action>) {
        for action in actions.drain(..) {
            self.perform_action(action);
        }
    }

    /// Set one action going. Which pool is the only thing decided here: a
    /// bank, an email and the store are three sources, so a slow one cannot
    /// crowd out the others.
    fn perform_action(self: &Arc<Self>, action: Action) {
        match action {
            // The answer somebody is parked on. There is no IO in handing it
            // over, and by now everything it was waiting on has landed — the
            // machine held it until the last write reported.
            Action::ToRouter { request_id, action } => {
                if self.answers.send((request_id, action)).is_err() {
                    tracing::error!(request_id, "the router is gone; its answer was dropped");
                }
            }

            Action::ToSource { .. } => self.calling(&self.plaid, action),

            Action::ToEmail { .. } => self.calling(&self.email, action),

            Action::ToStore { .. } => self.calling(&self.store, action),

            // No IO and nothing to wait for: a log line.
            Action::ToProcess { ref action } => self.perform_action_process(action),
        }
    }

    /// Hand one action to a pool, to be carried out whenever there is room.
    fn calling(self: &Arc<Self>, pool: &SourcePool, action: Action) {
        let runtime = Arc::clone(self);

        pool.call(async move { runtime.carry_out(&action).await });
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
    async fn carry_out(&self, action: &Action) -> Vec<Event> {
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

            ActionSource::FetchWebhookKey { call_id, kid } => {
                self.fetch_webhook_key(*call_id, kid).await
            }

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

    /// Ask Plaid for one webhook signing key, and report what came back.
    ///
    /// A key that will not fetch is reported as absent rather than as a
    /// failure: from the machine's side "Plaid has no such key" and "we could
    /// not ask" lead to the same refusal, and remembering both is what keeps
    /// an invented `kid` from costing a call every time it is sent.
    async fn fetch_webhook_key(&self, call_id: crate::CallId, kid: &str) -> Vec<Event> {
        let key = match runtime_plaid::webhook_key(&self.credentials, kid).await {
            Ok(key) => Some(key),

            Err(e) => {
                tracing::warn!(kid, error = %e, "could not fetch a webhook key");

                None
            }
        };

        vec![Event::FromSource {
            event: EventSource::WebhookKeyLoaded {
                call_id,
                kid: kid.to_string(),
                key,
            },
        }]
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

            ActionStore::LoadItem { call_id, item_id } => EventStore::ItemLoaded {
                call_id: *call_id,
                item: self.item_facts(item_id).await,
            },

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
            | ActionStore::LoadItem { .. }
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
            | ActionStore::LoadItem { .. }
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
            // Nothing to carry out. What stops this server is the loop
            // reading `stopped` once the machine has drained, and the machine
            // says so itself. The action is here because a runtime that had
            // something to wind down would wind it down on this.
            ActionProcess::Shutdown => {}

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
            let _ = self.landed.send(Event::FromSource {
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
