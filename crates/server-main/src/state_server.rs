//! The server's own decisions, kept away from its IO.
//!
//! Same shape as the app's machine and for the same reason
//! (`docs/ARCHITECTURE.md`): events in, actions out, nothing in between
//! touching the outside world.
//!
//! No data lives here. The machine holds what cannot be read back from
//! Postgres — how much of Plaid's rate limit has been spent, and where the
//! process is in its own life; everything else arrives on the event that needs
//! it and is served from the store.

use std::collections::HashMap;

use api_common::Timestamp;
use api_common::ratelimit::RateLimiter;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use server_wire::SyncStage;
use server_wire::WireSyncStatus;
use src_plaid::ITEM_LOGIN_REQUIRED;
use src_plaid::PlaidEndpoint;
use src_plaid::PlaidFault;
use src_plaid::PlaidRateCtx;
use src_plaid::PlaidRateLimiter;

use crate::Action;
use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionRouter;
use crate::ActionSocket;
use crate::ActionSource;
use crate::ActionStore;
use crate::CallId;
use crate::CensusServer;
use crate::Client;
use crate::ClientIp;
use crate::ConnId;
use crate::Event;
use crate::EventProcess;
use crate::EventRouter;
use crate::EventSocket;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;
use crate::ItemFacts;
use crate::PlaidCollect;
use crate::RequestId;
use crate::StoredCode;
use crate::WebhookFacts;
use crate::email_address;
use crate::login_code;
use crate::rate_limit::Route;
use crate::rate_limit::ServerRateCtx;
use crate::rate_limit::ServerRateLimiter;
use crate::session_token;
use src_plaid::PlaidJwk;
use src_plaid::PlaidWebhook;
use src_plaid::PlaidWebhookJwt;
use src_plaid::PlaidWebhookKind;
use uuid::Uuid;

/// How long a sync is good for before Plaid is asked again. Plaid
/// rate-limits `/accounts/balance/get` hard, and thirty seconds is short
/// enough that a pull-to-refresh still feels live.
pub const FRESH_FOR_SECS: u64 = 30;

/// How long to wait on a source before giving up on it. Not a latency budget
/// but the point past which a call has almost certainly gone quiet.
pub const CALL_GOOD_FOR_SECS: u64 = 30;

/// How long `/link/finish` waits for Plaid to say what a session came to.
///
/// The redirect that closes the sheet and Plaid's POST have no ordering
/// between them, so the route waits.
pub const LINK_ANSWER_GOOD_FOR_SECS: u64 = 10;

/// How long an outcome nobody has asked about is kept. A phone that has not
/// asked within ten minutes is not going to: the sheet is long closed.
pub const OUTCOME_GOOD_FOR_SECS: u64 = 10 * 60;

/// How long a session lasts. Long, because the alternative is asking somebody
/// to check their email to look at their own balance.
pub const SESSION_GOOD_FOR_SECS: u64 = 90 * 24 * 60 * 60;

/// How often the expired sessions are swept out of the store. Nothing depends
/// on it being prompt — an expired session signs nobody in either way — so
/// this is only about the table not growing.
pub const SWEEP_SESSIONS_EVERY_SECS: u64 = 60 * 60;

/// How long a `kid` Plaid would not answer for is remembered as a miss.
///
/// Short, because the two reasons for a miss want opposite things: an invented
/// `kid` should cost one call however often it is sent, and a real key we
/// failed to fetch must not be refused for the life of the process.
pub const WEBHOOK_KEY_MISS_GOOD_FOR_SECS: u64 = 5 * 60;

/// How many streams one user may hold open at once.
///
/// A rule rather than a socket count, so it is arithmetic the machine does and
/// an `eval!` test can reach. Generous for a person — a phone, a laptop and
/// room to spare — and small enough that a client reconnecting without letting
/// go cannot collect connections for the life of the process.
pub const WATCHERS_AT_MOST: usize = 10;

/// How many webhook signing keys are remembered at once. Finite because the
/// ids come off tokens strangers send: full means the misses are swept, and
/// still full means the webhook is refused.
pub const WEBHOOK_KEYS_AT_MOST: usize = 64;

/// A webhook signing key, or the fact that there was not one.
#[derive(Debug)]
struct CachedKey {
    /// `None` is a miss: Plaid did not answer for this id.
    key: Option<PlaidJwk>,

    /// When it was fetched. Only a miss expires; a key for a given `kid` is
    /// the same key forever, because rotation gives the new one a new id.
    at: Timestamp,
}

/// A sync that is running, and whose it is.
///
/// The owner is held rather than looked up: a sync outlives the request that
/// started it, and both places one starts have an Item and no session.
#[derive(Debug)]
struct Syncing {
    user_id: Uuid,

    /// Where this sync resumes from, and where its next page starts. Carried
    /// because the balances call goes first and this is not wanted until
    /// after it.
    cursor: Option<String>,

    /// Which of the calls is out.
    stage: SyncStage,

    /// Whether there is still a call out. A finished sync is kept rather than
    /// dropped: it is what a screen watching one reads to say it is done, and
    /// the totals under it.
    finished: bool,

    /// What the bank is called, once the balances call has said. It is in
    /// neither the Link session nor the exchange.
    institution: Option<String>,

    /// What this sync has turned up. Counted here because it exists nowhere
    /// else once the writes have gone out: these are what the sync brought,
    /// not what the store now holds.
    accounts: u32,
    transactions: u32,
    streams: u32,
}

impl Syncing {
    /// What a screen watching this sync is told.
    fn status(&self) -> WireSyncStatus {
        let mut status = WireSyncStatus::new(
            self.stage,
            self.institution.clone(),
            self.accounts,
            self.transactions,
            self.streams,
        );

        status.finished = self.finished;

        status
    }
}

/// A call in flight, and the route that wants its answer.
#[derive(Debug)]
struct Waiting {
    request_id: RequestId,

    /// When the call went out, so one that never answers can be given up on.
    since: Timestamp,
}

/// What a Link session came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkOutcome {
    Linked,

    /// The sheet was closed, or the exchange failed. The common ending.
    Nothing,
}

impl LinkOutcome {
    fn answer(self) -> ActionRouter {
        match self {
            LinkOutcome::Linked => ActionRouter::Linked { count: 1 },
            LinkOutcome::Nothing => ActionRouter::LinkAbandoned,
        }
    }
}

/// What a request is owed, once everything it started has landed.
///
/// Two shapes, because a snapshot is not decided until it is read: the read
/// for it must not go out until the writes it would race are done.
#[derive(Debug)]
enum Owed {
    Answer(ActionRouter),

    /// A read first, and the answer is whatever it finds.
    Snapshot {
        user_id: Uuid,
    },
}

/// What a request was doing, while a read it needs is out.
///
/// Only what the machine cannot get back: who is waiting is already in
/// [`StateServer::waiting`], keyed by the same call.
#[derive(Debug)]
enum Pending {
    /// A sign-in, waiting on the code the store holds against its address.
    Submitted { email: String, code: String },

    /// A request waiting on the session that says who is asking. The address
    /// rides along because the rate limit needs both halves of the caller.
    Authenticating { ip: ClientIp, event: Authenticated },

    /// A webhook waiting on the key it says it was signed with. Held whole,
    /// because nothing about it can be believed until the key arrives.
    WebhookKey { jwt: String, body: Vec<u8> },

    /// A believed webhook waiting on the bank it names.
    WebhookItem { item_id: String },

    /// A request whose caller is known, waiting on the rows it decides from.
    Loading { user_id: Uuid, event: Authenticated },
}

/// Which authenticated route is waiting, once there is somebody to answer it
/// for. One per [`EventRouter`](crate::EventRouter) variant carrying a token.
#[derive(Debug, Clone, Copy)]
enum Authenticated {
    LinkRequested,
    LinkClosed,

    /// The stream this request opens, carried through the session read so the
    /// answer knows which connection it is accepting.
    SyncWatchRequested {
        conn_id: ConnId,
    },

    SnapshotRequested,
}

impl Authenticated {
    /// The bucket a request for this counts against, once it has a caller.
    fn route(&self) -> Route {
        match self {
            Authenticated::LinkRequested => Route::LinkStart,
            Authenticated::LinkClosed => Route::LinkFinish,
            Authenticated::SyncWatchRequested { .. } => Route::SyncStatus,
            Authenticated::SnapshotRequested => Route::Snapshot,
        }
    }
}

/// An outcome nobody has asked about yet.
#[derive(Debug)]
struct Settled {
    outcome: LinkOutcome,

    /// When it was recorded, so one nobody collects does not sit here.
    at: Timestamp,
}

/// A route parked on a session Plaid has not reported on yet.
#[derive(Debug)]
struct Watching {
    request_id: RequestId,

    /// When it parked, so it can be let go rather than held.
    since: Timestamp,
}

/// Which authenticated route this event is, for the machine to come back to
/// once the session has resolved.
fn authenticated(event: &EventRouter) -> Option<Authenticated> {
    match event {
        EventRouter::LinkRequested { .. } => Some(Authenticated::LinkRequested),
        EventRouter::LinkClosed { .. } => Some(Authenticated::LinkClosed),
        EventRouter::SyncWatchRequested { conn_id, .. } => {
            Some(Authenticated::SyncWatchRequested { conn_id: *conn_id })
        }
        EventRouter::SnapshotRequested { .. } => Some(Authenticated::SnapshotRequested),
        _ => None,
    }
}

/// Where the server is in its own life. One way through, and both ends are
/// still.
#[derive(Debug, Default, PartialEq, Eq)]
enum State {
    /// Serving. Whether a credential can be minted is not this: that is
    /// whether there is a generator, and [`StateServer::rng`] holds it.
    #[default]
    Running,

    /// A signal has arrived. What is outstanding finishes; nothing new starts
    /// and nothing new is minted.
    Stopping,

    /// Nothing is outstanding and nothing more will be. The runtime's loop
    /// ends here, because only the machine knows what it is owed.
    Stopped,
}

/// Everything the server holds between requests.
///
/// Fields are private: state changes only through
/// [`handle_events`](StateServer::handle_events).
#[derive(Debug, Default)]
pub struct StateServer {
    state: State,

    /// Where sign-in codes and session tokens come from, seeded by
    /// [`EventProcess::Seeded`](crate::EventProcess). `Some` whenever the
    /// machine is [`State::Started`].
    ///
    /// ChaCha20 rather than a fast PRNG: a generator whose state can be
    /// recovered from its outputs hands an attacker every code after them.
    rng: Option<ChaCha20Rng>,

    /// What Plaid will let us send. A fact about this process's last minute,
    /// which is why it cannot be read back from the store.
    limiter: PlaidRateLimiter,

    /// What we will let a caller ask of us. Swept on the tick: its keys come
    /// from whoever calls us, so one that only inserted would be the thing
    /// that exhausts the server.
    requests: ServerRateLimiter,

    /// The next [`CallId`] to hand out. A counter rather than anything drawn,
    /// so the same log replays to the same numbers.
    next_call: CallId,

    /// Which route, if any, is parked on the answer to each call in flight.
    /// An absent entry is a call nobody is waiting on — a sync on a timer.
    waiting: HashMap<CallId, Waiting>,

    /// Answers that are decided but not yet owed. A route reads the store the
    /// moment it is told to, so one released while a write is in flight serves
    /// the rows that write was about to replace.
    held: HashMap<RequestId, Owed>,

    /// Requests waiting on a read, by the call that asked for it: what a
    /// request was has to outlive the turn it arrived in.
    pending: HashMap<CallId, Pending>,

    /// How far each running fetch has got, by item. Kept because a fetch is
    /// over by the time anything could read what it did; empty means idle.
    syncing: HashMap<String, Syncing>,

    /// Who is being written to, and on whose behalf. Keyed by the connection
    /// so that letting one go is a removal, and capped per user by
    /// [`WATCHERS_AT_MOST`].
    watchers: HashMap<ConnId, Uuid>,

    /// What each Link session came to, for a route that has not asked yet,
    /// keyed by the `link_token` that `SESSION_FINISHED` carries. Always empty
    /// under `PLAID_COLLECT=poll`, where the close causes the collection.
    outcomes: HashMap<String, Settled>,

    /// The other order: a route that asked before Plaid said anything.
    /// [`outcomes`](Self::outcomes) is the half that arrived first.
    watching: HashMap<String, Watching>,

    /// Which channel collects the result of a Link session, read out of the
    /// environment on [`EventProcess::Started`]. `None` is a machine that has
    /// not been told the process is up.
    collect: Option<PlaidCollect>,

    /// When the current batch is being handled.
    now: Timestamp,

    /// What is known about each webhook signing key, by the `kid` a token
    /// named. Swept on the tick: these names come from strangers.
    keys: HashMap<String, CachedKey>,

    /// When the store's expired sessions were last swept. The first tick
    /// starts the clock rather than sweeping, so a restart brings none on.
    swept: Option<Timestamp>,
}

impl StateServer {
    pub fn new() -> Self {
        Self::default()
    }

    /// The only entry point. Time is not an argument but an event, which is
    /// what makes a log of the events a recording of the session.
    pub fn handle_events(&mut self, events: &[Event], actions: &mut Vec<Action>) {
        for event in events {
            self.handle_event(event, actions);
        }

        // Only once every event in the batch has been handled.
        self.settle();
    }

    fn handle_event(&mut self, event: &Event, actions: &mut Vec<Action>) {
        match event {
            Event::FromRouter {
                request_id,
                client,
                event,
            } => self.handle_event_router(*request_id, client, event, actions),
            Event::FromSource { event } => self.handle_event_source(event, actions),
            Event::FromSocket { event } => self.handle_event_socket(event),
            Event::FromStore { call_id, event } => {
                self.handle_event_store(*call_id, event, actions)
            }
            Event::FromProcess { event } => self.handle_event_process(event, actions),
            Event::FromTerminal { event } => self.handle_event_terminal(event, actions),
        }
    }

    fn handle_event_router(
        &mut self,
        request_id: RequestId,
        client: &Client,
        event: &EventRouter,
        actions: &mut Vec<Action>,
    ) {
        // A route that has to know who is asking asks the store first.
        if let Some(token_sha256) = event.token() {
            let Some(authenticated) = authenticated(event) else {
                unreachable!("a variant carrying a token names what it is waiting for")
            };

            let call_id = self.calling(request_id);

            self.pending.insert(
                call_id,
                Pending::Authenticating {
                    ip: client.ip.clone(),
                    event: authenticated,
                },
            );

            actions.push(Action::ToStore {
                action: ActionStore::LoadSession {
                    call_id,
                    token_sha256: token_sha256.to_vec(),
                    now: self.now,
                },
            });

            return;
        }

        // Checked after the work would be a limit on how much we are thanked
        // for, not on how much we do.
        if let Some(route) = event.route() {
            // A botnet has an address each and one victim, so on the route
            // that sends mail the inbox is the window that sees one caller.
            let ctx = ServerRateCtx::new(route, client.clone()).mailing(event.recipient());

            // Where nothing can return early past it and leave a request
            // served but uncounted.
            if !self.requests.can_send(self.now, &ctx) {
                tracing::warn!(
                    client = %client.ip,
                    route = route.as_str(),
                    "a caller is over its rate limit",
                );

                self.answer(request_id, ActionRouter::TooMany, actions);

                return;
            }
        }

        match event {
            EventRouter::LinkRequested { .. } => {
                unreachable!("a request carrying a token is resolved before it is decided")
            }

            EventRouter::SyncWatchRequested { .. } | EventRouter::LinkClosed { .. } => {
                unreachable!("a request carrying a token is resolved before it is decided")
            }

            EventRouter::CodeRequested { email } => {
                // Answered as though it had been taken: the route says the
                // same thing whether or not an address has an account, and
                // the shape of the string must not say otherwise.
                if !email_address::plausible(email) {
                    tracing::warn!("a sign-in was asked for with something that is not an address",);

                    self.answer(request_id, ActionRouter::Accepted, actions);

                    return;
                }

                let Some(rng) = self.minting() else {
                    tracing::error!("asked for a sign-in code by a machine that mints none");

                    self.answer(request_id, ActionRouter::Unavailable, actions);

                    return;
                };

                let code = login_code::generate(rng);

                let expires_at =
                    Timestamp::from_secs(self.now.as_secs() + login_code::GOOD_FOR.as_secs());

                // Two actions because they go to different places, and only
                // one of them may ever hold the plaintext.
                actions.push(Action::ToStore {
                    action: ActionStore::SaveCode {
                        call_id: self.calling(request_id),
                        email: email.clone(),
                        code_sha256: login_code::hash(&code),
                        expires_at,
                    },
                });

                actions.push(Action::ToEmail {
                    action: ActionEmail::SendCode {
                        email: email.clone(),
                        code,
                    },
                });

                self.answer(request_id, ActionRouter::Accepted, actions);
            }

            EventRouter::CodeSubmitted { email, code } => {
                let call_id = self.calling(request_id);

                self.pending.insert(
                    call_id,
                    Pending::Submitted {
                        email: email.clone(),
                        code: code.clone(),
                    },
                );

                actions.push(Action::ToStore {
                    action: ActionStore::LoadCode {
                        call_id,
                        email: email.clone(),
                    },
                });
            }

            EventRouter::WebhookReceived { jwt, body } => {
                self.believe(request_id, jwt, body, actions)
            }

            EventRouter::SnapshotRequested { .. } => {
                unreachable!("a request carrying a token is resolved before it is decided")
            }
        }
    }

    /// Carry on with a request now that the store has said who is asking.
    ///
    /// The rate limit is asked here rather than on arrival, because charging
    /// the address first spends a slot the user's own window may refuse.
    fn handle_authenticated(
        &mut self,
        request_id: RequestId,
        ip: ClientIp,
        event: &Authenticated,
        user_id: Option<Uuid>,
        actions: &mut Vec<Action>,
    ) {
        // A token that names nobody: nothing to charge, nothing to serve.
        let Some(user_id) = user_id else {
            self.answer(request_id, ActionRouter::Unauthorized, actions);

            return;
        };

        let route = event.route();
        let ctx = ServerRateCtx::new(route, Client::new(ip.clone(), Some(user_id)));

        if !self.requests.can_send(self.now, &ctx) {
            tracing::warn!(
                client = %ip,
                route = route.as_str(),
                "a caller is over its rate limit",
            );

            self.answer(request_id, ActionRouter::TooMany, actions);

            return;
        }

        // The rows this route decides from, scoped to the caller: two users
        // share a process and share no cursors and no sessions.
        let read = match event {
            // Nothing to read: the id is the whole of what it needed.
            Authenticated::LinkRequested => {
                actions.push(Action::ToSource {
                    action: ActionSource::OpenLink {
                        call_id: self.calling(request_id),
                        user_id,
                    },
                });

                return;
            }

            // Nothing to read either: what is running is the machine's own
            // state, and it is the only thing that knows.
            Authenticated::SyncWatchRequested { conn_id } => {
                self.handle_sync_watch(request_id, *conn_id, user_id, actions);

                return;
            }

            Authenticated::SnapshotRequested => {
                let call_id = self.calling(request_id);

                ActionStore::LoadItems { call_id, user_id }
            }

            Authenticated::LinkClosed => {
                let call_id = self.calling(request_id);

                ActionStore::LoadPendingLinks { call_id, user_id }
            }
        };

        self.pending.insert(
            read.call_id(),
            Pending::Loading {
                user_id,
                event: *event,
            },
        );

        actions.push(Action::ToStore { action: read });
    }

    /// Somebody asked for their ledger. Which banks are worth going back to
    /// is decided here; the ledger is read once those calls land, so a
    /// snapshot never serves rows a fetch of its own was replacing.
    fn handle_snapshot(
        &mut self,
        request_id: RequestId,
        user_id: Uuid,
        items: &[ItemFacts],
        actions: &mut Vec<Action>,
    ) {
        let asked: Vec<_> = items
            .iter()
            // Never forced: this asks for the ledger, not a trip to the bank.
            .filter(|item| self.should_fetch(item, false))
            .map(|item| (item.item_id.clone(), item.cursor.clone()))
            .collect();

        for (item_id, cursor) in asked {
            let call_id = self.calling(request_id);

            self.fetch(call_id, item_id, user_id, cursor, actions);
        }

        // Read once the last of those has landed and been written, so what is
        // served is the rows they wrote. A user with no banks asked nothing,
        // so the read goes out now and serves the empty ledger.
        self.owe(request_id, Owed::Snapshot { user_id }, actions);
    }

    /// Somebody wants to be told how their sync goes.
    fn handle_sync_watch(
        &mut self,
        request_id: RequestId,
        conn_id: ConnId,
        user_id: Uuid,
        actions: &mut Vec<Action>,
    ) {
        let open = self
            .watchers
            .values()
            .filter(|watcher| **watcher == user_id)
            .count();

        // Refused rather than closing the oldest: a client that cannot let go
        // is a client whose oldest connection is the one still working.
        if open >= WATCHERS_AT_MOST {
            tracing::warn!(%user_id, open, "a caller is holding too many streams open");

            self.answer(request_id, ActionRouter::TooMany, actions);

            return;
        }

        self.watchers.insert(conn_id, user_id);

        let status = self.status_for(user_id);

        self.answer(
            request_id,
            ActionRouter::Watching { conn_id, status },
            actions,
        );
    }

    /// A stream went away.
    fn handle_event_socket(&mut self, event: &EventSocket) {
        match event {
            EventSocket::Closed { conn_id } => {
                // Absent is ordinary: one refused for the cap was never
                // written down, and the connection still closes.
                self.watchers.remove(conn_id);
            }
        }
    }

    /// How far this user's sync has got.
    ///
    /// The furthest-behind of their running fetches: the screen draws one set
    /// of rows, and the honest stage is the unfinished one.
    fn status_for(&self, user_id: Uuid) -> WireSyncStatus {
        self.syncing
            .values()
            .filter(|syncing| syncing.user_id == user_id)
            // One still running outranks one that finished at the same stage,
            // or a second bank landing would be reported as already done.
            .min_by_key(|syncing| (syncing.finished, stage_index(syncing.stage)))
            .map_or_else(WireSyncStatus::idle, Syncing::status)
    }

    /// Tell everyone watching this user where their sync has got to.
    ///
    /// The whole status every time. A stream that carried changes would leave
    /// a client that reconnected mid-sync holding a picture it could not
    /// complete.
    fn push_status(&mut self, user_id: Uuid, actions: &mut Vec<Action>) {
        let status = self.status_for(user_id);

        for (conn_id, watcher) in &self.watchers {
            if *watcher != user_id {
                continue;
            }

            actions.push(Action::ToSocket {
                action: ActionSocket::Send {
                    conn_id: *conn_id,
                    status: status.clone(),
                },
            });
        }
    }

    /// A sheet closed. What that means depends on how results are collected.
    fn handle_link_closed(
        &mut self,
        request_id: RequestId,
        link_tokens: &[String],
        actions: &mut Vec<Action>,
    ) {
        // Collection already happens unprompted, so this route asks rather
        // than triggers: Plaid would charge a call for a token already gone.
        if self.collects_by_webhook() {
            self.handle_link_closed_by_webhook(request_id, link_tokens, actions);

            return;
        }

        // Nothing open, so nothing to go and ask about.
        if link_tokens.is_empty() {
            self.answer(request_id, ActionRouter::LinkAbandoned, actions);

            return;
        }

        actions.push(Action::ToSource {
            action: ActionSource::PollLinks {
                call_id: self.calling(request_id),
                link_tokens: link_tokens.to_vec(),
            },
        });
    }

    /// Check a code against what the store held, and sign somebody in or not.
    /// A turn later than the submission, which is why the machine kept what
    /// they typed.
    fn verify_code(
        &mut self,
        request_id: RequestId,
        email: &str,
        code: &str,
        stored: Option<&StoredCode>,
        actions: &mut Vec<Action>,
    ) {
        // No code standing against this address, and a rejection like any
        // other: telling them apart says which addresses have one in flight.
        let Some(stored) = stored else {
            self.answer(request_id, ActionRouter::Rejected, actions);

            return;
        };

        let verdict = login_code::verify(stored, code, self.now);

        if verdict != login_code::Verdict::Accepted {
            // Counted whatever the reason, expiry included: resetting the cap
            // by waiting would give unlimited guesses at the next code.
            actions.push(Action::ToStore {
                action: ActionStore::RecordAttempt {
                    call_id: self.calling(request_id),
                    email: email.to_string(),
                },
            });

            tracing::warn!(email, ?verdict, "a sign-in code was rejected");

            self.answer(request_id, ActionRouter::Rejected, actions);

            return;
        }

        let Some(rng) = self.minting() else {
            tracing::error!("a code was accepted by a machine that mints none");

            self.answer(request_id, ActionRouter::Rejected, actions);

            return;
        };

        let token = login_code::generate_token(rng);

        actions.push(Action::ToStore {
            action: ActionStore::CreateSession {
                call_id: self.calling(request_id),
                email: email.to_string(),
                token_sha256: session_token::hash(&token),
                expires_at: Timestamp::from_secs(self.now.as_secs() + SESSION_GOOD_FOR_SECS),
            },
        });

        // Single use, and spent only once it has bought something.
        actions.push(Action::ToStore {
            action: ActionStore::DeleteCode {
                call_id: self.calling(request_id),
                email: email.to_string(),
            },
        });

        self.answer(request_id, ActionRouter::SignedIn { token }, actions);
    }

    /// A sheet closed, on a server Plaid calls.
    ///
    /// The redirect that closed it says nothing — Plaid's documentation is
    /// explicit that reaching the completion URI is not a connection, and that
    /// the answer is `SESSION_FINISHED` — so this route only asks.
    fn handle_link_closed_by_webhook(
        &mut self,
        request_id: RequestId,
        pending: &[String],
        actions: &mut Vec<Action>,
    ) {
        // Looked for before anybody is parked, because the POST usually wins
        // the race and waiting first would make it sit until it timed out.
        if let Some((link_token, outcome)) = self.collected(pending) {
            self.report(request_id, &link_token, outcome, actions);

            return;
        }

        // Nothing open and nothing recorded: `Pending` would promise nothing.
        if pending.is_empty() {
            self.answer(request_id, ActionRouter::LinkAbandoned, actions);

            return;
        }

        // Parked on every session the caller has open. The first to report
        // answers, and `handle_timeouts` is what stops this being forever.
        for link_token in pending {
            let displaced = self.watching.insert(
                link_token.clone(),
                Watching {
                    request_id,
                    since: self.now,
                },
            );

            // Two calls about the same session: the older is let go rather
            // than parked on a key it does not own, which nothing answers.
            if let Some(displaced) = displaced
                && displaced.request_id != request_id
            {
                self.answer(displaced.request_id, ActionRouter::LinkPending, actions);
            }
        }
    }

    /// The recorded outcome for one of these sessions, taken as it is read. A
    /// bank attached wins over one that was not: somebody who backed out of
    /// their first bank and connected a second has two sessions.
    fn collected(&mut self, pending: &[String]) -> Option<(String, LinkOutcome)> {
        let found = pending
            .iter()
            .filter_map(|link_token| Some((link_token, self.outcomes.get(link_token)?.outcome)))
            .max_by_key(|(_, outcome)| *outcome == LinkOutcome::Linked)
            .map(|(link_token, outcome)| (link_token.clone(), outcome))?;

        self.outcomes.remove(&found.0);

        Some(found)
    }

    /// Tell one route what a session came to, and close the session.
    ///
    /// A collected session is one `/link/finish` cannot see, so collecting it
    /// as it is reported is what makes a second close answer nothing.
    fn report(
        &mut self,
        request_id: RequestId,
        link_token: &str,
        outcome: LinkOutcome,
        actions: &mut Vec<Action>,
    ) {
        // An entry left behind sends the next settlement to a dead request.
        self.watching
            .retain(|_, watching| watching.request_id != request_id);

        actions.push(Action::ToStore {
            action: ActionStore::CollectLink {
                call_id: self.calling(request_id),
                link_token: link_token.to_string(),
            },
        });

        self.answer(request_id, outcome.answer(), actions);
    }

    /// One Link session has come to something, on a server Plaid calls.
    /// Either a route is parked on it or one will ask, and this is the one
    /// place that does not care which.
    fn settled(&mut self, link_token: &str, outcome: LinkOutcome, actions: &mut Vec<Action>) {
        // No token means `/link/start` failed before there was one.
        if link_token.is_empty() {
            return;
        }

        let Some(watching) = self.watching.remove(link_token) else {
            self.outcomes.insert(
                link_token.to_string(),
                Settled {
                    outcome,
                    at: self.now,
                },
            );

            return;
        };

        self.report(watching.request_id, link_token, outcome, actions);
    }

    /// Whether Plaid calls us with the result of a Link session.
    fn collects_by_webhook(&self) -> bool {
        self.collect == Some(PlaidCollect::Webhook)
    }

    /// Whether this really is Plaid, and if so what it said.
    ///
    /// Every check is a rule rather than mechanics — which algorithm is
    /// acceptable, how long an unheard-of `kid` stays refused — so it belongs
    /// here rather than at the edge.
    fn believe(
        &mut self,
        request_id: RequestId,
        jwt: &str,
        body: &[u8],
        actions: &mut Vec<Action>,
    ) {
        // Everything free before anything expensive: refusing the wrong
        // algorithm stops a made-up `kid` reaching a call to Plaid.
        let parsed = match PlaidWebhookJwt::parse(jwt) {
            Ok(parsed) => parsed,

            Err(rejection) => {
                tracing::warn!(%rejection, "a webhook was not believed");

                self.answer(request_id, ActionRouter::Unauthorized, actions);

                return;
            }
        };

        match self.known(parsed.kid()) {
            Some(Some(key)) => self.verified(request_id, &parsed, &key, body, actions),

            // Refused without asking Plaid again, the reason a miss is kept.
            Some(None) => {
                tracing::warn!(
                    kid = parsed.kid(),
                    "a webhook named a key plaid does not have"
                );

                self.answer(request_id, ActionRouter::Unauthorized, actions);
            }

            None => self.fetch_webhook_key(request_id, parsed.kid(), jwt, body, actions),
        }
    }

    /// Go and ask Plaid for a key, if there is room to remember the answer.
    /// The one place an unauthenticated request can cause an outbound call.
    fn fetch_webhook_key(
        &mut self,
        request_id: RequestId,
        kid: &str,
        jwt: &str,
        body: &[u8],
        actions: &mut Vec<Action>,
    ) {
        if self.keys.len() >= WEBHOOK_KEYS_AT_MOST {
            tracing::error!(
                kid,
                "refusing to fetch another webhook key: too many have been asked for",
            );

            self.answer(request_id, ActionRouter::Unauthorized, actions);

            return;
        }

        let call_id = self.calling(request_id);

        self.pending.insert(
            call_id,
            Pending::WebhookKey {
                jwt: jwt.to_string(),
                body: body.to_vec(),
            },
        );

        actions.push(Action::ToSource {
            action: ActionSource::FetchWebhookKey {
                call_id,
                kid: kid.to_string(),
            },
        });
    }

    /// The signature holds. What did it say? The body is read only now: one
    /// parsed before its hash was checked is a body a stranger wrote.
    fn verified(
        &mut self,
        request_id: RequestId,
        jwt: &PlaidWebhookJwt,
        key: &PlaidJwk,
        body: &[u8],
        actions: &mut Vec<Action>,
    ) {
        if let Err(rejection) = jwt.verify(key, body, self.now.as_secs() as i64) {
            tracing::warn!(%rejection, "a webhook was not believed");

            self.answer(request_id, ActionRouter::Unauthorized, actions);

            return;
        }

        let webhook: PlaidWebhook = match serde_json::from_slice(body) {
            Ok(webhook) => webhook,

            Err(e) => {
                tracing::error!(error = %e, "plaid signed a body that is not JSON");

                self.answer(request_id, ActionRouter::Unauthorized, actions);

                return;
            }
        };

        tracing::info!(
            webhook_type = webhook.webhook_type,
            webhook_code = webhook.webhook_code,
            "a webhook arrived from plaid",
        );

        let facts = match webhook.kind() {
            PlaidWebhookKind::SessionFinished {
                link_token,
                public_tokens,
            } => WebhookFacts::SessionFinished {
                link_token,
                public_tokens,
            },

            // The one kind that needs a read: it names a bank, not a person.
            PlaidWebhookKind::SyncUpdatesAvailable { item_id } => {
                let call_id = self.calling(request_id);

                self.pending.insert(
                    call_id,
                    Pending::WebhookItem {
                        item_id: item_id.clone(),
                    },
                );

                actions.push(Action::ToStore {
                    action: ActionStore::LoadItem { call_id, item_id },
                });

                return;
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
        };

        self.handle_webhook(request_id, &facts, actions);
    }

    /// What is already known about this `kid`: `Some(Some(key))` is a hit,
    /// `Some(None)` a miss still standing, and `None` nothing known — the
    /// only one that goes on to ask Plaid.
    fn known(&self, kid: &str) -> Option<Option<PlaidJwk>> {
        let cached = self.keys.get(kid)?;

        if cached.key.is_none()
            && self.now.since(cached.at).as_secs() >= WEBHOOK_KEY_MISS_GOOD_FOR_SECS
        {
            return None;
        }

        Some(cached.key.clone())
    }

    /// Drop the misses that have expired, on the tick rather than when the
    /// next webhook arrives, so invented ids cannot keep the map full.
    fn sweep_webhook_keys(&mut self) {
        let now = self.now;

        self.keys.retain(|_, cached| {
            cached.key.is_some() || now.since(cached.at).as_secs() < WEBHOOK_KEY_MISS_GOOD_FOR_SECS
        });
    }

    /// What a webhook Plaid sent us is worth doing about. Every one of them
    /// answers 2xx, because Plaid retries anything else.
    fn handle_webhook(
        &mut self,
        request_id: RequestId,
        webhook: &WebhookFacts,
        actions: &mut Vec<Action>,
    ) {
        match webhook {
            WebhookFacts::SessionFinished {
                link_token,
                public_tokens,
            } => {
                // Finished with nothing: they closed the sheet. An outcome
                // Plaid *said*, not one inferred from an empty poll.
                if public_tokens.is_empty() {
                    self.answer(request_id, ActionRouter::Accepted, actions);

                    self.settled(link_token, LinkOutcome::Nothing, actions);

                    return;
                }

                // Every token, because Plaid says this once: one nobody
                // exchanges is a bank somebody connected and does not get.
                for public_token in public_tokens {
                    actions.push(Action::ToSource {
                        action: ActionSource::ExchangePublicToken {
                            call_id: self.calling(request_id),
                            link_token: link_token.clone(),
                            public_token: public_token.clone(),
                        },
                    });
                }

                // Held until the exchanges have landed and been written, so a
                // failure mid-exchange is a webhook Plaid sends again.
                self.answer(request_id, ActionRouter::Accepted, actions);
            }

            WebhookFacts::SyncUpdatesAvailable { item_id, item } => {
                // News about a bank the store has no row for. Nothing to fix
                // from here, but the two sides disagreeing is worth saying.
                let Some(item) = item else {
                    tracing::warn!(item_id, "plaid has news about a bank we do not hold");

                    self.answer(request_id, ActionRouter::Accepted, actions);

                    return;
                };

                // Forced: the webhook *is* the news the freshness window was
                // waiting for. The rate limit and a dead login still apply.
                if self.should_fetch(item, true) {
                    // Nobody waits on it: a first sync of a long history
                    // takes far longer than Plaid waits for its 2xx.
                    let call_id = self.call();

                    self.fetch(
                        call_id,
                        item.item_id.clone(),
                        item.user_id,
                        item.cursor.clone(),
                        actions,
                    );
                }

                self.answer(request_id, ActionRouter::Accepted, actions);
            }

            WebhookFacts::Login { item_id, required } => {
                if *required {
                    actions.push(Action::ToProcess {
                        action: ActionProcess::ReportLoginRequired,
                    });
                }

                // Written down, or the server comes back up and asks Plaid
                // the same question it was just refused.
                actions.push(Action::ToStore {
                    action: ActionStore::SaveStatus {
                        call_id: self.calling(request_id),
                        item_id: item_id.clone(),
                        error_code: required.then(|| ITEM_LOGIN_REQUIRED.to_string()),
                    },
                });

                self.answer(request_id, ActionRouter::Accepted, actions);
            }

            WebhookFacts::Ignored {
                webhook_type,
                webhook_code,
            } => {
                tracing::debug!(webhook_type, webhook_code, "a webhook nothing acts on");

                self.answer(request_id, ActionRouter::Accepted, actions);
            }
        }
    }

    fn handle_event_source(&mut self, event: &EventSource, actions: &mut Vec<Action>) {
        match event {
            EventSource::WebhookKeyLoaded { call_id, kid, key } => {
                // A miss that is not written down is a call to Plaid for
                // every POST naming an id nobody has.
                self.keys.insert(
                    kid.clone(),
                    CachedKey {
                        key: key.clone(),
                        at: self.now,
                    },
                );

                let Some(Pending::WebhookKey { jwt, body }) = self.pending.remove(call_id) else {
                    tracing::error!(call_id, "a key answered a webhook nobody is waiting on");

                    return;
                };

                let Some(request_id) = self.answered(*call_id) else {
                    return;
                };

                let Some(key) = key else {
                    tracing::warn!(kid, "a webhook named a key plaid does not have");

                    self.answer(request_id, ActionRouter::Unauthorized, actions);

                    return;
                };

                // Parsed again rather than kept: base64 twice is cheaper than
                // a variant holding a borrow across turns.
                let Ok(parsed) = PlaidWebhookJwt::parse(&jwt) else {
                    unreachable!("this token parsed on the turn it arrived")
                };

                self.verified(request_id, &parsed, key, &body, actions);
            }

            EventSource::BalancesLoaded {
                call_id,
                item_id,
                connection,
            } => {
                if let Some(syncing) = self.syncing.get_mut(item_id) {
                    syncing.institution = Some(connection.name.clone());
                    syncing.accounts = connection.accounts.len().try_into().unwrap_or(u32::MAX);
                    syncing.stage = SyncStage::Transactions;

                    let user_id = syncing.user_id;

                    self.push_status(user_id, actions);
                }

                // Written down by the call that brought it, whatever becomes
                // of the rest of the sync. Nothing here holds a ledger to
                // assign over, the trap `docs/PLAID.md` describes.
                actions.push(Action::ToStore {
                    action: ActionStore::SaveAccounts {
                        call_id: self.alongside(*call_id),
                        item_id: item_id.clone(),
                        institution_name: Some(connection.name.clone()),
                        accounts: connection.accounts.clone(),
                    },
                });

                // A sync nothing is recording is one that was given up on, so
                // there is nothing to ask Plaid next.
                let Some(cursor) = self.syncing.get(item_id).map(|s| s.cursor.clone()) else {
                    self.landed(*call_id, actions);

                    return;
                };

                actions.push(Action::ToSource {
                    action: ActionSource::SyncTransactions {
                        call_id: self.passing(*call_id),
                        item_id: item_id.clone(),
                        cursor,
                    },
                });
            }

            // The lists are a **diff** handed straight to the store.
            EventSource::TransactionsLoaded {
                call_id,
                item_id,
                added,
                modified,
                removed,
                cursor,
                has_more,
            } => {
                // A page beyond the first spends a slot of its own. Plaid
                // meters `/transactions/sync` per Item per minute and a first
                // history is several pages, so a chain that asked nobody
                // would be the one way past the budget. Stopping costs
                // little: the write below saves the cursor to resume from.
                let another_page =
                    *has_more && self.may_call(PlaidEndpoint::TransactionsSync, item_id);

                if *has_more && !another_page {
                    tracing::warn!(
                        item_id,
                        "out of budget part-way through a history; the next sync resumes",
                    );
                }

                if let Some(syncing) = self.syncing.get_mut(item_id) {
                    syncing.transactions = syncing
                        .transactions
                        .saturating_add(added.len().try_into().unwrap_or(u32::MAX));

                    syncing.cursor = Some(cursor.clone());

                    if !another_page {
                        syncing.stage = SyncStage::Streams;
                    }

                    let user_id = syncing.user_id;

                    self.push_status(user_id, actions);
                }

                actions.push(Action::ToStore {
                    action: ActionStore::SaveTransactions {
                        call_id: self.alongside(*call_id),
                        item_id: item_id.clone(),
                        added: added.clone(),
                        modified: modified.clone(),
                        removed: removed.clone(),
                        cursor: cursor.clone(),
                    },
                });

                // A sync nothing is recording is one that was given up on.
                let Some(cursor) = self.syncing.get(item_id).map(|s| s.cursor.clone()) else {
                    self.landed(*call_id, actions);

                    return;
                };

                let call_id = self.passing(*call_id);
                let item_id = item_id.clone();

                let action = if another_page {
                    ActionSource::SyncTransactions {
                        call_id,
                        item_id,
                        cursor,
                    }
                } else {
                    ActionSource::FetchRecurring { call_id, item_id }
                };

                actions.push(Action::ToSource { action });
            }

            EventSource::RecurringLoaded {
                call_id,
                item_id,
                streams,
            } => {
                if let Some(syncing) = self.syncing.get_mut(item_id) {
                    syncing.streams = streams.len().try_into().unwrap_or(u32::MAX);
                }

                self.sync_finished(item_id, actions);

                // The wait passes from the last call to the last write, so
                // the route is answered once the rows are there.
                actions.push(Action::ToStore {
                    action: ActionStore::SaveStreams {
                        call_id: self.passing(*call_id),
                        item_id: item_id.clone(),
                        streams: streams.clone(),
                    },
                });
            }

            EventSource::LinkOpened {
                call_id,
                user_id,
                link_token,
                hosted_link_url,
            } => {
                let Some(request_id) = self.answered(*call_id) else {
                    return;
                };

                // Written down first, or somebody could finish a session we
                // have no record of asking for.
                actions.push(Action::ToStore {
                    action: ActionStore::SaveLink {
                        call_id: self.calling(request_id),
                        link_token: link_token.clone(),
                        user_id: *user_id,
                    },
                });

                self.answer(
                    request_id,
                    ActionRouter::LinkOpened {
                        url: hosted_link_url.clone(),
                    },
                    actions,
                );
            }

            EventSource::LinksPolled { call_id, polls } => {
                let Some(request_id) = self.answered(*call_id) else {
                    return;
                };

                // A token is the answer whether or not the session says it
                // has finished. The first wins and the rest wait for the next
                // close, because the route can be told about one.
                let exchangeable = polls
                    .iter()
                    .find_map(|poll| Some((&poll.link_token, poll.public_tokens.first()?)));

                if let Some((link_token, public_token)) = exchangeable {
                    actions.push(Action::ToSource {
                        action: ActionSource::ExchangePublicToken {
                            call_id: self.calling(request_id),
                            link_token: link_token.clone(),
                            public_token: public_token.clone(),
                        },
                    });

                    return;
                }

                // They closed the sheet, and it is marked done so it is not
                // polled forever. One still running is somebody typing.
                for poll in polls.iter().filter(|poll| poll.finished) {
                    actions.push(Action::ToStore {
                        action: ActionStore::CollectLink {
                            call_id: self.calling(request_id),
                            link_token: poll.link_token.clone(),
                        },
                    });
                }

                self.answer(request_id, ActionRouter::LinkAbandoned, actions);
            }

            EventSource::ItemLinked {
                call_id,
                link_token,
                item_id,
            } => {
                let Some(request_id) = self.answered(*call_id) else {
                    return;
                };

                // The first sync follows this write rather than racing it:
                // whose bank it is comes from the Link session, and the write
                // is what resolves that.
                actions.push(Action::ToStore {
                    action: ActionStore::SaveItem {
                        call_id: self.calling(request_id),
                        item_id: item_id.clone(),
                        link_token: link_token.clone(),
                    },
                });

                // A webhook arrives unprompted, so the session has to stand
                // until `/link/finish` reports it.
                if !self.collects_by_webhook() {
                    actions.push(Action::ToStore {
                        action: ActionStore::CollectLink {
                            call_id: self.calling(request_id),
                            link_token: link_token.clone(),
                        },
                    });
                }

                // Plaid's POST caused this and is owed nothing beyond
                // "taken": the outcome goes to `/link/finish`.
                if self.collects_by_webhook() {
                    self.answer(request_id, ActionRouter::Accepted, actions);

                    self.settled(link_token, LinkOutcome::Linked, actions);

                    return;
                }

                self.linked_one(request_id, actions);
            }

            EventSource::LinkAbandoned {
                call_id,
                link_token,
            } => {
                let Some(request_id) = self.answered(*call_id) else {
                    return;
                };

                // An exchange that failed, on a session Plaid told us about:
                // nothing came of it, and somebody is waiting to hear that.
                if self.collects_by_webhook() && !link_token.is_empty() {
                    self.answer(request_id, ActionRouter::Accepted, actions);

                    self.settled(link_token, LinkOutcome::Nothing, actions);

                    return;
                }

                actions.push(Action::ToStore {
                    action: ActionStore::CollectLink {
                        call_id: self.calling(request_id),
                        link_token: link_token.clone(),
                    },
                });

                self.answer(request_id, ActionRouter::LinkAbandoned, actions);
            }

            EventSource::PlaidFailed {
                call_id,
                item_id,
                stage,
                fault,
            } => {
                // The recurring call is the one a sync can do without.
                // Transactions are what the app is for, and a missing list of
                // charges costs one screen rather than all of them.
                if *stage == SyncStage::Streams {
                    tracing::warn!(
                        item_id,
                        code = fault.as_str(),
                        "recurring unavailable, the sync stands without it",
                    );

                    self.sync_finished(item_id, actions);
                    self.landed(*call_id, actions);

                    return;
                }

                self.sync_stopped(item_id, actions);

                if *fault == PlaidFault::LoginRequired {
                    actions.push(Action::ToProcess {
                        action: ActionProcess::ReportLoginRequired,
                    });

                    actions.push(Action::ToStore {
                        action: ActionStore::SaveStatus {
                            call_id: self.passing(*call_id),
                            item_id: item_id.clone(),
                            error_code: Some(fault.as_str().to_string()),
                        },
                    });

                    return;
                }

                // Nothing to write, so the route is not left hanging.
                self.landed(*call_id, actions);
            }
        }
    }

    fn handle_event_process(&mut self, event: &EventProcess, actions: &mut Vec<Action>) {
        match event {
            // Nothing is pulled into memory at boot: the store is read per
            // request, which is what makes a second server process possible.
            EventProcess::Started { collect } => self.collect = Some(*collect),

            EventProcess::Seeded { seed } => {
                // A seed on the way out is dropped: taking it would put a
                // stopping machine back in service.
                if self.shutting_down() {
                    return;
                }

                self.rng = Some(ChaCha20Rng::from_seed(*seed));
            }

            EventProcess::Tick { now } => {
                // Time only ever moves forward: a stale reading would make a
                // freshness window wrong in a way nothing else here catches.
                debug_assert!(
                    *now >= self.now,
                    "time went backwards: {now} after {}",
                    self.now,
                );

                self.now = *now;

                // Or the limiters' maps grow for the life of the process:
                // one is keyed by whoever calls us and the other by every bank
                // we have ever fetched.
                self.requests.sweep(self.now);
                self.limiter.sweep(self.now);
                self.sweep_webhook_keys();

                self.handle_timeouts(actions);

                // Last, so a route let go of is decided before housekeeping.
                self.sweep_sessions(actions);
            }
        }
    }

    fn handle_event_store(
        &mut self,
        call_id: CallId,
        event: &EventStore,
        actions: &mut Vec<Action>,
    ) {
        match event {
            EventStore::CodeLoaded { stored } => {
                let Some(Pending::Submitted { email, code }) = self.pending.remove(&call_id) else {
                    tracing::error!(call_id, "the store answered a sign-in nobody submitted");

                    return;
                };

                let Some(request_id) = self.answered(call_id) else {
                    return;
                };

                self.verify_code(request_id, &email, &code, stored.as_ref(), actions);
            }

            EventStore::ItemLoaded { item } => {
                let Some(Pending::WebhookItem { item_id }) = self.pending.remove(&call_id) else {
                    tracing::error!(call_id, "the store answered a bank nobody asked about");

                    return;
                };

                let Some(request_id) = self.answered(call_id) else {
                    return;
                };

                self.handle_webhook(
                    request_id,
                    &WebhookFacts::SyncUpdatesAvailable {
                        item_id,
                        item: item.clone(),
                    },
                    actions,
                );
            }

            EventStore::ItemSaved { item } => {
                // Now rather than on the next `/snapshot`: the first sync of a
                // new bank is the long one, and a screen watching a sync needs
                // one running. Nobody waits on it — `/link/finish` would stay
                // open for the whole of it.
                if let Some(item) = item {
                    let call_id = self.call();

                    // A bank attached seconds ago has no cursor to resume from.
                    self.fetch(call_id, item.item_id.clone(), item.user_id, None, actions);
                }

                self.landed(call_id, actions);
            }

            EventStore::ItemsLoaded { items } => {
                let Some(Pending::Loading { user_id, event }) = self.pending.remove(&call_id)
                else {
                    tracing::error!(call_id, "the store answered banks nobody asked about");

                    return;
                };

                let Some(request_id) = self.answered(call_id) else {
                    return;
                };

                match event {
                    Authenticated::SnapshotRequested => {
                        self.handle_snapshot(request_id, user_id, items, actions)
                    }

                    _ => unreachable!("only a snapshot asks for banks"),
                }
            }

            EventStore::PendingLinksLoaded { link_tokens } => {
                let Some(Pending::Loading { .. }) = self.pending.remove(&call_id) else {
                    tracing::error!(call_id, "the store answered sessions nobody asked about");

                    return;
                };

                let Some(request_id) = self.answered(call_id) else {
                    return;
                };

                self.handle_link_closed(request_id, link_tokens, actions);
            }

            EventStore::SnapshotLoaded { snapshot } => {
                let Some(request_id) = self.answered(call_id) else {
                    return;
                };

                self.answer(
                    request_id,
                    ActionRouter::Serve {
                        snapshot: snapshot.clone(),
                    },
                    actions,
                );
            }

            EventStore::SessionResolved { user_id } => {
                let Some(Pending::Authenticating { ip, event }) = self.pending.remove(&call_id)
                else {
                    tracing::error!(call_id, "the store answered a session nobody asked about");

                    return;
                };

                let Some(request_id) = self.answered(call_id) else {
                    return;
                };

                self.handle_authenticated(request_id, ip, &event, *user_id, actions);
            }

            // There is no answer to fall back on: a sign-in refused because
            // the store was down is indistinguishable from a wrong code.
            EventStore::ReadError { message } => {
                tracing::error!(message, "could not read the store");

                self.pending.remove(&call_id);

                let Some(request_id) = self.answered(call_id) else {
                    return;
                };

                self.answer(request_id, ActionRouter::Unavailable, actions);
            }

            EventStore::WriteSuccess => self.landed(call_id, actions),

            // Ends the wait: a route held for a failed write is held for
            // ever.
            EventStore::WriteError { message } => {
                tracing::error!(message, "could not write the store");

                self.landed(call_id, actions);
            }
        }
    }

    fn handle_event_terminal(&mut self, event: &EventTerminal, actions: &mut Vec<Action>) {
        match event {
            EventTerminal::SigInt | EventTerminal::SigTerm => {
                // A second ^C while the first drains queues no second one.
                if self.shutting_down() {
                    return;
                }

                tracing::info!("shutting down");

                self.state = State::Stopping;

                actions.push(Action::ToProcess {
                    action: ActionProcess::Shutdown,
                });
            }
        }
    }

    /// What a route is owed, sent the moment nothing it started is still out.
    /// Every answer goes through here, so no route has to remember to wait.
    fn answer(&mut self, request_id: RequestId, action: ActionRouter, actions: &mut Vec<Action>) {
        self.owe(request_id, Owed::Answer(action), actions);
    }

    /// Hold what a request is owed until everything it started has landed.
    fn owe(&mut self, request_id: RequestId, owed: Owed, actions: &mut Vec<Action>) {
        if self.outstanding(request_id) {
            self.held.insert(request_id, owed);

            return;
        }

        self.release(request_id, owed, actions);
    }

    /// Nothing is outstanding, so this is what the request gets. A snapshot
    /// is one more call rather than an answer: the rows it reads are the ones
    /// the calls that just landed wrote.
    fn release(&mut self, request_id: RequestId, owed: Owed, actions: &mut Vec<Action>) {
        match owed {
            Owed::Answer(action) => actions.push(Action::ToRouter { request_id, action }),

            Owed::Snapshot { user_id } => {
                let call_id = self.calling(request_id);

                actions.push(Action::ToStore {
                    action: ActionStore::LoadSnapshot { call_id, user_id },
                });
            }
        }
    }

    /// One call has landed. Release the answer it was the last thing holding.
    /// Silent when nothing was waiting, unlike [`answered`](Self::answered):
    /// that is the ordinary shape of a sync nobody asked for.
    fn landed(&mut self, call_id: CallId, actions: &mut Vec<Action>) {
        let Some(Waiting { request_id, .. }) = self.waiting.remove(&call_id) else {
            return;
        };

        // Somebody with three banks is answered after the third, not the first.
        if self.outstanding(request_id) {
            return;
        }

        let Some(owed) = self.held.remove(&request_id) else {
            return;
        };

        self.release(request_id, owed, actions);
    }

    /// Start a sync of one Item, and remember that we have — both in the same
    /// turn, or a screen watching it reads the interval where nothing says so
    /// as "finished".
    ///
    /// Only the first of the calls goes out here. What follows it is decided
    /// when each answer lands, which is what puts the order, the paging and
    /// the budget in one place a test can reach.
    fn fetch(
        &mut self,
        call_id: CallId,
        item_id: String,
        user_id: Uuid,
        cursor: Option<String>,
        actions: &mut Vec<Action>,
    ) {
        // Nothing is known yet. What matters is that it is *not* finished.
        self.syncing.insert(
            item_id.clone(),
            Syncing {
                user_id,
                cursor,
                stage: SyncStage::Accounts,
                finished: false,
                institution: None,
                accounts: 0,
                transactions: 0,
                streams: 0,
            },
        );

        // In the same turn it starts: a screen that opened its stream while
        // nothing was running is told that something now is.
        self.push_status(user_id, actions);

        actions.push(Action::ToSource {
            action: ActionSource::FetchBalances {
                call_id,
                item_id,
                now: self.now,
            },
        });
    }

    /// Whether Plaid's budget for this endpoint and this bank has room, and
    /// spend the slot if it has.
    fn may_call(&mut self, endpoint: PlaidEndpoint, item_id: &str) -> bool {
        let now = self.now;

        self.limiter
            .can_send(now, &PlaidRateCtx::new(endpoint, item_id))
    }

    /// One more bank attached on this request. Counted rather than set,
    /// because one `/link/finish` can attach two banks, turns apart; the count
    /// rides on the held answer rather than a tally that outlives it.
    fn linked_one(&mut self, request_id: RequestId, actions: &mut Vec<Action>) {
        let count = match self.held.get(&request_id) {
            Some(Owed::Answer(ActionRouter::Linked { count })) => *count,
            _ => 0,
        };

        self.answer(
            request_id,
            ActionRouter::Linked { count: count + 1 },
            actions,
        );
    }

    /// Whether this route started anything that has not come back.
    fn outstanding(&self, request_id: RequestId) -> bool {
        self.waiting
            .values()
            .any(|waiter| waiter.request_id == request_id)
    }

    /// A sync is over and what it brought stands. Kept rather than dropped:
    /// this is what a screen watching one reads to say it is done, and the
    /// totals it draws under that.
    ///
    /// A sync with no record of having started is one that was given up on.
    /// What its calls brought is written down all the same by whatever else
    /// the event asked for; there is just nobody it is known to belong to.
    fn sync_finished(&mut self, item_id: &str, actions: &mut Vec<Action>) {
        let Some(syncing) = self.syncing.get_mut(item_id) else {
            return;
        };

        syncing.finished = true;
        syncing.stage = SyncStage::Streams;

        let user_id = syncing.user_id;

        self.push_status(user_id, actions);
    }

    /// Stop reporting a sync. Dropped rather than marked done: a screen
    /// watching one that died would otherwise sit on its last stage.
    fn sync_stopped(&mut self, item_id: &str, actions: &mut Vec<Action>) {
        let Some(syncing) = self.syncing.remove(item_id) else {
            return;
        };

        self.push_status(syncing.user_id, actions);
    }

    /// A fresh [`CallId`] owed to the same route as `call_id`, which keeps
    /// its own wait. For an answer that causes two: this is the branch that
    /// does not carry the chain on, and the other takes the wait with
    /// [`passing`](Self::passing).
    fn alongside(&mut self, call_id: CallId) -> CallId {
        match self.waiting.get(&call_id) {
            Some(waiter) => {
                let request_id = waiter.request_id;

                self.calling(request_id)
            }
            None => self.call(),
        }
    }

    /// A fresh [`CallId`] that inherits whatever `call_id` was holding. One
    /// call causes the next, and the route waits for the end of that chain.
    fn passing(&mut self, call_id: CallId) -> CallId {
        match self.waiting.remove(&call_id) {
            Some(waiter) => self.calling(waiter.request_id),
            None => self.call(),
        }
    }

    /// A fresh [`CallId`], noted as owed to the route that is waiting.
    fn calling(&mut self, request_id: RequestId) -> CallId {
        let call_id = self.call();

        self.waiting.insert(
            call_id,
            Waiting {
                request_id,
                since: self.now,
            },
        );

        call_id
    }

    /// A fresh [`CallId`] for a call nobody is waiting on.
    fn call(&mut self) -> CallId {
        self.next_call += 1;

        self.next_call
    }

    /// Who was waiting on this call, now that it has answered. Absent means
    /// an answer to a question we have no record of asking, and picking a
    /// route for it would send somebody another person's bank.
    fn answered(&mut self, call_id: CallId) -> Option<RequestId> {
        let Some(Waiting { request_id, .. }) = self.waiting.remove(&call_id) else {
            tracing::error!(call_id, "a source answered a call nobody is waiting on");

            return None;
        };

        Some(request_id)
    }

    /// Ask the store to drop the sessions that have expired, about hourly.
    /// Decided here rather than on a runtime timer, so a log replays it.
    fn sweep_sessions(&mut self, actions: &mut Vec<Action>) {
        // Nothing new starts once we are on the way out.
        if self.shutting_down() {
            return;
        }

        // Nothing is owed a sweep at boot: expired rows are already inert.
        let Some(swept) = self.swept else {
            self.swept = Some(self.now);

            return;
        };

        if self.now.since(swept).as_secs() < SWEEP_SESSIONS_EVERY_SECS {
            return;
        }

        self.swept = Some(self.now);

        actions.push(Action::ToStore {
            action: ActionStore::SweepSessions {
                call_id: self.call(),
                now: self.now,
            },
        });
    }

    /// Stop waiting on calls that have taken too long.
    ///
    /// The reason there is a tick at all: a source that hangs rather than
    /// fails produces no event, and the route waiting on it would be parked
    /// forever. It answers with whatever was decided, and
    /// [`ActionRouter::Unavailable`] when nothing was.
    fn handle_timeouts(&mut self, actions: &mut Vec<Action>) {
        let now = self.now;

        self.handle_link_timeouts(actions);

        let given_up: Vec<CallId> = self
            .waiting
            .iter()
            .filter(|(_, waiter)| now.since(waiter.since).as_secs() >= CALL_GOOD_FOR_SECS)
            .map(|(call_id, _)| *call_id)
            .collect();

        for call_id in given_up {
            let Some(Waiting { request_id, .. }) = self.waiting.remove(&call_id) else {
                unreachable!("the key came from the map")
            };

            tracing::warn!(
                call_id,
                request_id,
                "a call took too long and was given up on"
            );

            // What the request was doing goes with the call it was waiting
            // on. Nothing else ever removes this: an answer is what does, and
            // giving up is deciding there will not be one.
            self.pending.remove(&call_id);

            // Another of this route's calls may still answer and release it.
            if self.outstanding(request_id) {
                continue;
            }

            // A snapshot still reads and still serves: a call that never
            // answered costs the fresher rows, not the ledger already there.
            let owed = self
                .held
                .remove(&request_id)
                .unwrap_or(Owed::Answer(ActionRouter::Unavailable));

            self.release(request_id, owed, actions);
        }
    }

    /// Let go of the routes waiting on a Link session Plaid has not reported,
    /// and drop the outcomes nobody ever came back for.
    ///
    /// `Pending` rather than "nothing came of it", because neither is known
    /// yet: the screen that follows it polls `/sync/status`, so a bank landing
    /// a moment later still shows up.
    fn handle_link_timeouts(&mut self, actions: &mut Vec<Action>) {
        let now = self.now;

        self.outcomes
            .retain(|_, settled| now.since(settled.at).as_secs() < OUTCOME_GOOD_FOR_SECS);

        let mut waited_out: Vec<RequestId> = self
            .watching
            .values()
            .filter(|watching| now.since(watching.since).as_secs() >= LINK_ANSWER_GOOD_FOR_SECS)
            .map(|watching| watching.request_id)
            .collect();

        // One route parked on two sessions times out twice, owed one answer.
        waited_out.sort_unstable();
        waited_out.dedup();

        for request_id in waited_out {
            self.watching
                .retain(|_, watching| watching.request_id != request_id);

            tracing::info!(
                request_id,
                "plaid has not said what a link session came to yet",
            );

            self.answer(request_id, ActionRouter::LinkPending, actions);
        }
    }

    /// Whether to go back to Plaid about this bank.
    ///
    /// `force` is the machine's alone, and only a webhook sets it: reachable
    /// from the wire, a gesture anybody can repeat becomes a way to spend this
    /// account's Plaid budget.
    fn should_fetch(&mut self, item: &ItemFacts, force: bool) -> bool {
        // Nothing new starts once we are on the way out.
        if self.shutting_down() {
            return false;
        }

        // No amount of asking fixes a login, so this outranks even `force`.
        if item.login_required {
            return false;
        }

        let stale = match item.synced_at {
            None => true,
            Some(at) => self.now.since(at).as_secs() >= FRESH_FOR_SECS,
        };

        if !force && !stale {
            return false;
        }

        // A sync is three calls, and the strictest budget decides. Asked last
        // so a request refused for freshness does not spend a slot. A history
        // long enough to page asks again for each page after the first.
        let now = self.now;
        let item_id = item.item_id.as_str();

        [
            PlaidEndpoint::TransactionsSync,
            PlaidEndpoint::BalanceGet,
            PlaidEndpoint::RecurringGet,
        ]
        .iter()
        .all(|endpoint| {
            self.limiter
                .can_send(now, &PlaidRateCtx::new(*endpoint, item_id))
        })
    }

    /// Whether the machine is on its way out, drained or not.
    /// The generator, when the machine may draw a credential from it.
    ///
    /// Two conditions in one place: entropy has arrived, and we are not on the
    /// way out. A predictable sign-in code is a way into somebody's bank, and
    /// one minted by a process that is about to exit is a code nobody can use.
    fn minting(&mut self) -> Option<&mut ChaCha20Rng> {
        if self.shutting_down() {
            return None;
        }

        self.rng.as_mut()
    }

    /// How much the machine is holding. See [`CensusServer`].
    ///
    /// The one thing about this machine no action reports, and so the only way
    /// a test can say that a flow gave back everything it took.
    pub fn census(&self) -> CensusServer {
        CensusServer {
            waiting: self.waiting.len(),
            held: self.held.len(),
            pending: self.pending.len(),
            syncing: self.syncing.len(),
            watchers: self.watchers.len(),
            outcomes: self.outcomes.len(),
            watching: self.watching.len(),
            keys: self.keys.len(),
            request_windows: self.requests.kept(),
            plaid_windows: self.limiter.kept(),
        }
    }

    pub fn shutting_down(&self) -> bool {
        matches!(self.state, State::Stopping | State::Stopped)
    }

    /// Whether there is nothing left to do. The runtime's loop runs until this.
    pub fn stopped(&self) -> bool {
        self.state == State::Stopped
    }

    /// Come to a stop, once there is nothing left that could still answer. At
    /// the end of every batch rather than on the tick alone, so a signal on an
    /// idle server stops it in the turn it arrives in.
    fn settle(&mut self) {
        if self.state != State::Stopping {
            return;
        }

        let outstanding = !self.waiting.is_empty()
            || !self.pending.is_empty()
            || !self.held.is_empty()
            || !self.watching.is_empty();

        if outstanding {
            return;
        }

        tracing::info!("stopped");

        self.state = State::Stopped;
    }
}

/// Where a stage sits in the order they run in, so two can be compared.
fn stage_index(stage: SyncStage) -> usize {
    SyncStage::ALL
        .iter()
        .position(|s| *s == stage)
        .unwrap_or_default()
}
