//! The server's own decisions, kept away from its IO.
//!
//! Same shape as the app's machine and for the same reason
//! (`docs/ARCHITECTURE.md`): events in, actions out, nothing in between
//! touching the outside world.
//!
//! What is *not* here any more is data. The machine used to hold the ledger,
//! the connections and the streams, and answer `/snapshot` from them — which
//! was only ever right because there was one user. It now holds what cannot be
//! read back from Postgres: how much of Plaid's rate limit has been spent, and
//! where the process is in its own life. Everything else arrives on the event
//! that needs it, and is served from the store.
//!
//! So "should I sync this bank?" is a pure function of facts handed in and a
//! clock handed in, which is what makes a stale-cache bug reproducible from a
//! log rather than from a stopwatch.

use std::collections::HashMap;

use api_common::Timestamp;
use api_common::ratelimit::RateLimiter;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use server_wire::SyncStage;
use server_wire::WireSyncStatus;
use src_plaid::ITEM_LOGIN_REQUIRED;
use src_plaid::PlaidEndpoint;
use src_plaid::PlaidRateCtx;
use src_plaid::PlaidRateLimiter;

use crate::Action;
use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionRouter;
use crate::ActionSource;
use crate::ActionStore;
use crate::CallId;
use crate::Client;
use crate::Event;
use crate::EventProcess;
use crate::EventRouter;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;
use crate::ItemFacts;
use crate::PlaidCollect;
use crate::RequestId;
use crate::WebhookFacts;
use crate::login_code;
use crate::rate_limit::ServerRateCtx;
use crate::rate_limit::ServerRateLimiter;
use crate::session_token;

/// How long a sync is good for before Plaid is asked again.
///
/// Plaid rate-limits `/accounts/balance/get` hard, and a phone that refetched
/// on every page load would hit it. Thirty seconds is short enough that a
/// pull-to-refresh feels live and long enough that flicking between tabs costs
/// nothing.
pub const FRESH_FOR_SECS: u64 = 30;

/// Everything the server holds between requests.
///
/// Fields are private: state changes only through
/// [`handle_events`](StateServer::handle_events).
/// How long to wait on a source before giving up on it.
///
/// Generous: a cold Plaid sync of a long history is slow, and cutting one off
/// costs a real answer. It is not a latency budget — it is the point past
/// which a call has almost certainly gone quiet rather than gone slow.
pub const CALL_GOOD_FOR_SECS: u64 = 30;

/// How long `/link/finish` waits for Plaid to say what a session came to.
///
/// The two events that end a Link session — the browser being redirected, and
/// Plaid's POST to us — have no ordering between them, and this server is the
/// only party that sees both. So the route waits, and this is how long
/// somebody is left looking at a spinner before being moved on to a screen
/// that is doing something instead. Ten seconds: long enough that a webhook
/// held up by a slow network still lands inside it, short enough not to read
/// as a hang.
pub const LINK_ANSWER_GOOD_FOR_SECS: u64 = 10;

/// How long an outcome nobody has asked about is kept.
///
/// Plaid's POST usually beats the phone, so an outcome is written down for
/// whoever asks next. A phone that has not asked within ten minutes is not
/// going to: the sheet is long closed and the flow it belonged to is gone.
pub const OUTCOME_GOOD_FOR_SECS: u64 = 10 * 60;

/// How long a session lasts. Long, because the alternative is asking somebody
/// to check their email to look at their own balance; short enough that a
/// phone lost in a drawer stops working eventually.
pub const SESSION_GOOD_FOR_SECS: u64 = 90 * 24 * 60 * 60;

/// A call in flight, and the route that wants its answer.
#[derive(Debug)]
struct Waiting {
    request_id: RequestId,

    /// When the call went out, so one that never answers can be given up on.
    /// Without it a source that goes quiet is a route parked forever.
    since: Timestamp,
}

/// What a Link session came to.
///
/// Two answers, and Plaid says which — `SESSION_FINISHED` carries a `status`
/// of `SUCCESS` or `EXITED`. That is the change webhooks bring to this: "no
/// bank" used to be inferred from a poll that came back empty, and is now
/// something we were told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkOutcome {
    /// A bank is attached.
    Linked,

    /// The session ended without one — the sheet was closed, or the exchange
    /// failed. The common ending, not a rare one.
    Nothing,
}

impl LinkOutcome {
    /// What the route waiting on it is told.
    fn answer(self) -> ActionRouter {
        match self {
            LinkOutcome::Linked => ActionRouter::Linked { count: 1 },
            LinkOutcome::Nothing => ActionRouter::Linked { count: 0 },
        }
    }
}

/// An outcome nobody has asked about yet.
#[derive(Debug)]
struct Settled {
    outcome: LinkOutcome,

    /// When it was recorded, so one nobody ever collects does not sit here
    /// for the life of the process.
    at: Timestamp,
}

/// A route parked on a session Plaid has not reported on yet.
#[derive(Debug)]
struct Watching {
    request_id: RequestId,

    /// When it parked, so it can be let go rather than held until the phone
    /// gives up on its own.
    since: Timestamp,
}

/// Where the server is in its own life.
#[derive(Debug, Default, PartialEq, Eq)]
enum State {
    /// Nothing has happened yet: the process has not said it is up.
    #[default]
    Stopped,

    /// Up, and without entropy. Nothing that needs a credential can be
    /// answered.
    Starting,

    /// Up and seeded. Everything the machine does is available.
    Started,

    /// A signal has arrived. What is outstanding finishes; nothing new starts
    /// and nothing new is minted.
    Stopping,
}

#[derive(Debug, Default)]
pub struct StateServer {
    state: State,

    /// Where sign-in codes and session tokens come from, seeded from the edge
    /// by [`EventProcess::Seeded`](crate::EventProcess).
    ///
    /// ChaCha20 rather than a fast PRNG: these are credentials, and a
    /// generator whose state can be recovered from a couple of outputs would
    /// hand an attacker every code after the ones they saw.
    ///
    /// `Some` whenever the machine is [`State::Started`] — that is what being
    /// started means — so the two places that draw from it check the state and
    /// take the generator out of the option without a second answer to give.
    rng: Option<ChaCha20Rng>,

    /// What Plaid will let us send. The one thing here that genuinely cannot
    /// be read back from the store — it is a fact about this process's last
    /// minute, and it is why "may I call?" is arithmetic in the machine rather
    /// than a guess in the runtime.
    limiter: PlaidRateLimiter,

    /// What we will let a caller ask of us, which is the same question in the
    /// other direction and for the same reason: a fact about the last minute,
    /// held nowhere else, and a rule rather than mechanics.
    ///
    /// Swept on the tick. Its keys come from whoever calls us, so a limiter
    /// that only ever inserted would be the thing that exhausts the server
    /// (#40).
    requests: ServerRateLimiter,

    /// The next [`CallId`] to hand out. A counter rather than anything drawn,
    /// so the same log replays to the same numbers.
    next_call: CallId,

    /// Which route, if any, is parked on the answer to each call in flight.
    ///
    /// The one thing here that is bookkeeping rather than a decision, and it
    /// has to be: the answer to a call arrives turns later, on an event that
    /// has no idea who asked. An absent entry is a call nobody is waiting on —
    /// a sync on a timer — which is why this is a lookup and not an `Option`
    /// on the event.
    waiting: HashMap<CallId, Waiting>,

    /// Answers that are decided but not yet owed.
    ///
    /// A route reads the store the moment it is told to, so an answer released
    /// while a write is still in flight serves the rows that write was about to
    /// replace. Everything a request set going has to land first — which is
    /// one rule rather than a note on each route to remember to wait.
    held: HashMap<RequestId, ActionRouter>,

    /// How far each running fetch has got, by item.
    ///
    /// The one piece of state here that exists purely to be looked at. It is
    /// kept because it cannot be recovered: a fetch is over by the time
    /// anything could go and read what it did, so the first-run screen that
    /// watches one has this or it has nothing (#25).
    ///
    /// An entry appears on the first
    /// [`PlaidProgressed`](crate::EventSource) and goes when the fetch answers
    /// or fails. Empty therefore means nothing is running, which is what a
    /// user who is not syncing is told.
    syncing: HashMap<String, WireSyncStatus>,

    /// What each Link session came to, for a route that has not asked yet.
    ///
    /// Keyed by `link_token`, which `SESSION_FINISHED` carries and
    /// `link_sessions` already stores against the user who opened it — so
    /// matching a webhook to the route waiting on it needs nothing new in the
    /// schema.
    ///
    /// Always empty under `PLAID_COLLECT=poll`: there the close is what
    /// *causes* the collection, so there is never a later asker.
    outcomes: HashMap<String, Settled>,

    /// The other order: a route that asked before Plaid said anything.
    ///
    /// The redirect that closes the sheet and the POST that carries the result
    /// race, and either can win. This is the half that is waiting, and
    /// [`outcomes`](Self::outcomes) is the half that arrived first.
    watching: HashMap<String, Watching>,

    /// Which channel collects the result of a Link session, as the runtime
    /// read it out of the environment on [`EventProcess::Started`].
    ///
    /// A fact from the edge, like the clock and the seed, rather than
    /// something the runtime consults on its own — see
    /// `docs/ARCHITECTURE.md`. `None` is a machine that has not been told the
    /// process is up, which is a machine that has not been asked anything
    /// either.
    collect: Option<PlaidCollect>,

    /// When the current batch is being handled.
    now: Timestamp,
}

impl StateServer {
    pub fn new() -> Self {
        Self::default()
    }

    /// The only entry point.
    ///
    /// Time is not an argument. It arrives as
    /// [`EventProcess::Tick`](crate::EventProcess) like anything else the
    /// machine cannot go and get for itself, which is what makes a log of the
    /// events a recording of the session rather than a list of things that
    /// happened at whatever time the replay runs at.
    pub fn handle_events(&mut self, events: &[Event], actions: &mut Vec<Action>) {
        for event in events {
            self.handle_event(event, actions);
        }
    }

    fn handle_event(&mut self, event: &Event, actions: &mut Vec<Action>) {
        match event {
            Event::FromRouter {
                request_id,
                client,
                event,
            } => self.handle_event_router(*request_id, client, event, actions),
            Event::FromSource { event } => self.handle_event_source(event, actions),
            Event::FromStore { event } => self.handle_event_store(event, actions),
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
        // Before anything is read, decided or sent. A route that is over its
        // limit costs one map lookup and a 429, which is the only way a limit
        // is worth having — checked after the work is a limit on how much we
        // are thanked for, not on how much we do.
        //
        // A route with no bucket is not asked about. There is one, and
        // [`EventRouter::route`](crate::EventRouter::route) is where it is
        // named and justified.
        if let Some(route) = event.route() {
            let ctx = ServerRateCtx::new(route, client.clone());

            // Asking counts, so this happens once and where nothing can return
            // early past it and leave a request served but uncounted.
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
            // Free to ask for, and creates nothing at Plaid's end — a session
            // nobody finishes costs nothing and expires on its own. So there
            // is nothing to rate limit here that is not already covered by
            // issue #23.
            EventRouter::LinkRequested { user_id } => {
                actions.push(Action::ToSource {
                    action: ActionSource::OpenLink {
                        call_id: self.calling(request_id),
                        user_id: *user_id,
                    },
                });
            }

            EventRouter::SyncStatusRequested { items } => {
                // The furthest-behind of this user's running fetches, because
                // the screen draws one set of rows and the honest thing for it
                // to show is the stage that has not finished yet. First run
                // has exactly one bank, so this is a rule for later rather
                // than one anybody meets today.
                let status = items
                    .iter()
                    .filter_map(|item| self.syncing.get(&item.item_id))
                    .min_by_key(|status| stage_index(status.stage()))
                    .map_or_else(WireSyncStatus::idle, Clone::clone);

                self.answer(request_id, ActionRouter::SyncStatus { status }, actions);
            }

            EventRouter::LinkClosed { pending } => {
                // Collection already happens, unprompted, so this route sets
                // nothing going: it stops being a trigger and becomes a
                // question. Asking Plaid again would spend a call to be told
                // about a token that has already been exchanged.
                if self.collects_by_webhook() {
                    self.handle_link_closed_by_webhook(request_id, pending, actions);

                    return;
                }

                // Nothing open. Somebody closed a sheet we have no record of,
                // or collected this one already — either way there is nothing
                // to go and ask about.
                if pending.is_empty() {
                    self.answer(request_id, ActionRouter::Linked { count: 0 }, actions);

                    return;
                }

                actions.push(Action::ToSource {
                    action: ActionSource::PollLinks {
                        call_id: self.calling(request_id),
                        link_tokens: pending.clone(),
                    },
                });
            }

            EventRouter::CodeRequested { email } => {
                // No entropy, no credentials. Refused rather than filled in
                // with anything derivable — a predictable sign-in code is a
                // way into somebody's bank, and there is no fallback that is
                // better than saying no. A machine on its way out refuses for
                // the same reason it starts no fetches.
                if self.state != State::Started {
                    tracing::error!(
                        state = ?self.state,
                        "asked for a sign-in code by a machine that is not started",
                    );

                    self.answer(request_id, ActionRouter::Unavailable, actions);

                    return;
                }

                let Some(rng) = self.rng.as_mut() else {
                    unreachable!("a started machine has a generator")
                };

                let code = login_code::generate(rng);

                let expires_at =
                    Timestamp::from_secs(self.now.as_secs() + login_code::GOOD_FOR.as_secs());

                // The hash is stored and the code is sent. They are separate
                // actions because they go to different places, and only one of
                // them may ever hold the plaintext.
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

            EventRouter::CodeSubmitted {
                email,
                code,
                stored,
            } => {
                // No code standing against this address. A rejection like any
                // other: telling them apart would say which addresses have a
                // sign-in in flight.
                let Some(stored) = stored else {
                    self.answer(request_id, ActionRouter::Rejected, actions);

                    return;
                };

                let verdict = login_code::verify(stored, code, self.now);

                if verdict != login_code::Verdict::Accepted {
                    // Counted whatever the reason, including expired: an
                    // attacker who could reset the cap by waiting would have
                    // an unlimited number of guesses at the next code.
                    actions.push(Action::ToStore {
                        action: ActionStore::RecordAttempt {
                            call_id: self.calling(request_id),
                            email: email.clone(),
                        },
                    });

                    tracing::warn!(email, ?verdict, "a sign-in code was rejected");

                    self.answer(request_id, ActionRouter::Rejected, actions);

                    return;
                }

                if self.state != State::Started {
                    tracing::error!(
                        state = ?self.state,
                        "a code was accepted by a machine that is not started",
                    );

                    self.answer(request_id, ActionRouter::Rejected, actions);

                    return;
                }

                let Some(rng) = self.rng.as_mut() else {
                    unreachable!("a started machine has a generator")
                };

                let token = login_code::generate_token(rng);

                actions.push(Action::ToStore {
                    action: ActionStore::CreateSession {
                        call_id: self.calling(request_id),
                        email: email.clone(),
                        token_sha256: session_token::hash(&token),
                        expires_at: Timestamp::from_secs(
                            self.now.as_secs() + SESSION_GOOD_FOR_SECS,
                        ),
                    },
                });

                // Single use, and spent only once it has bought something.
                actions.push(Action::ToStore {
                    action: ActionStore::DeleteCode {
                        call_id: self.calling(request_id),
                        email: email.clone(),
                    },
                });

                self.answer(request_id, ActionRouter::SignedIn { token }, actions);
            }

            EventRouter::WebhookReceived { webhook } => {
                self.handle_webhook(request_id, webhook, actions)
            }

            EventRouter::SnapshotRequested { force, items } => {
                let asked: Vec<_> = items
                    .iter()
                    .filter(|item| self.should_fetch(item, *force))
                    .map(|item| (item.item_id.clone(), item.cursor.clone()))
                    .collect();

                for (item_id, cursor) in asked {
                    let call_id = self.calling(request_id);

                    self.fetch(call_id, item_id, cursor, actions);
                }

                // Decided here and released when the last of those has landed
                // and been written. A user with no banks asked for nothing, so
                // it goes out on this turn — the empty ledger rather than an
                // error, which is what "no bank linked yet" looks like and the
                // state first run exists to leave.
                self.answer(request_id, ActionRouter::Serve, actions);
            }
        }
    }

    /// A sheet closed, on a server Plaid calls.
    ///
    /// The redirect that closed it says nothing — Plaid's own documentation is
    /// explicit that reaching the completion URI is not a successful
    /// connection, and that the answer is `SESSION_FINISHED` or
    /// `/link/token/get`. So this route no longer counts anything. It asks
    /// what became of the session, and the answer is either already here or
    /// about to be.
    fn handle_link_closed_by_webhook(
        &mut self,
        request_id: RequestId,
        pending: &[String],
        actions: &mut Vec<Action>,
    ) {
        // Looked for before anybody is parked, because the POST usually wins
        // the race: the phone has a redirect, an app switch and a request to
        // get through, and Plaid has none of that. Waiting first would make
        // the ordinary case sit until it timed out.
        if let Some((link_token, outcome)) = self.collected(pending) {
            self.report(request_id, &link_token, outcome, actions);

            return;
        }

        // Nothing open and nothing recorded. There is no session to hear
        // about, so `Pending` would be a promise nothing can keep.
        if pending.is_empty() {
            self.answer(request_id, ActionRouter::Linked { count: 0 }, actions);

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

            // Two calls about the same session. The older one is let go
            // rather than left parked on a key it no longer owns, which would
            // be a request nothing ever answers.
            if let Some(displaced) = displaced
                && displaced.request_id != request_id
            {
                self.answer(displaced.request_id, ActionRouter::LinkPending, actions);
            }
        }
    }

    /// The recorded outcome for one of these sessions, taken as it is read.
    ///
    /// A bank attached wins over one that was not: somebody who backed out of
    /// their first bank and connected a second has two sessions, and the one
    /// that worked is the answer to give.
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
    /// The two happen together, and that is the point: a collected session is
    /// one `/link/finish` can no longer see, so it is collected by the call
    /// that reports it and not a moment earlier. That is also what makes a
    /// second close of the same session answer nothing rather than the same
    /// bank twice.
    fn report(
        &mut self,
        request_id: RequestId,
        link_token: &str,
        outcome: LinkOutcome,
        actions: &mut Vec<Action>,
    ) {
        // Everything else this route was parked on. It is owed one answer and
        // is about to get it, and an entry left behind would send the next
        // settlement to a request that has gone.
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
    ///
    /// Either a route is parked on it, or one will ask — those are the two
    /// orders the race can come out in, and this is the one place that does
    /// not care which.
    fn settled(&mut self, link_token: &str, outcome: LinkOutcome, actions: &mut Vec<Action>) {
        // A session with no token is `/link/start` failing before there was
        // one, which is not a session anybody can be waiting on.
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

    /// What a webhook Plaid sent us is worth doing about.
    ///
    /// Every one of them answers, and answers 2xx: Plaid retries anything
    /// else, so an error here is the same webhook arriving again in a minute
    /// and again after that. What differs is what is set going first.
    ///
    /// Nothing about *believing* the webhook happens here. It is already
    /// believed by the time it is an event — see
    /// [`WebhookFacts`](crate::WebhookFacts) — the same way a `user_id` on any
    /// other router event is already a session that checked out.
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
                // Finished with nothing: they closed the sheet. An outcome and
                // not a failure — and for the first time it is something Plaid
                // *said* rather than something inferred from a poll that came
                // back empty, which is what lets "That didn't finish" be true
                // rather than a guess.
                if public_tokens.is_empty() {
                    self.answer(request_id, ActionRouter::Accepted, actions);

                    self.settled(link_token, LinkOutcome::Nothing, actions);

                    return;
                }

                // Every token, where the polling path takes the first and
                // leaves the rest for the next close to collect. There is no
                // next close here — Plaid says this once — so a token nobody
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

                // Held until the exchanges have landed and been written, by
                // the ordinary rule. That is what makes a failure mid-exchange
                // a webhook Plaid sends again rather than a bank quietly lost.
                self.answer(request_id, ActionRouter::Accepted, actions);
            }

            WebhookFacts::SyncUpdatesAvailable { item_id, item } => {
                // Plaid is still telling us about a bank the store has no row
                // for. Nothing to sync and nothing to fix from here; said
                // rather than dropped, because the two sides disagreeing is
                // worth knowing about.
                let Some(item) = item else {
                    tracing::warn!(item_id, "plaid has news about a bank we do not hold");

                    self.answer(request_id, ActionRouter::Accepted, actions);

                    return;
                };

                // Forced: the webhook *is* the news that something changed, so
                // the freshness window has nothing left to say. What still
                // applies is the rate limit and a dead login, and
                // `should_fetch` is where both of those live.
                if self.should_fetch(item, true) {
                    // Nobody waits on it. A first sync of a long history takes
                    // far longer than Plaid waits for its 2xx, and a webhook
                    // Plaid decides went unanswered is one it sends again.
                    let call_id = self.call();

                    self.fetch(call_id, item.item_id.clone(), item.cursor.clone(), actions);
                }

                self.answer(request_id, ActionRouter::Accepted, actions);
            }

            WebhookFacts::Login { item_id, required } => {
                if *required {
                    actions.push(Action::ToProcess {
                        action: ActionProcess::ReportLoginRequired,
                    });
                }

                // Written down, or the server comes back up and asks Plaid the
                // same question it was just refused. This is also what the
                // Reconnect button reads to know it has something to offer.
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
            // The lists are a **diff** — what changed since the cursor — and
            // they are handed straight to the store, which applies them. The
            // machine no longer keeps a ledger to apply them to, which is what
            // removed the trap `docs/PLAID.md` describes rather than guarding
            // against it: there is nothing here to assign over.
            EventSource::PlaidLoaded {
                call_id,
                item_id,
                added,
                modified,
                removed,
                connection,
                streams,
                cursor,
            } => {
                // Over — and what it brought, which is the last thing anybody
                // watching it needs. Replaced rather than dropped because
                // these counts exist nowhere else: `added` is what this sync
                // brought, not what the store now holds, and the two stop
                // agreeing the moment there is a second sync.
                //
                // It stays until the next fetch for this item overwrites it.
                // One entry per bank is not growth worth managing, and a late
                // poller reading the last sync's numbers is being told
                // something true.
                self.syncing.insert(
                    item_id.clone(),
                    WireSyncStatus::done(
                        connection.as_ref().map(|c| c.name.clone()),
                        connection
                            .as_ref()
                            .map_or(0, |c| c.accounts.len().try_into().unwrap_or(u32::MAX)),
                        added.len().try_into().unwrap_or(u32::MAX),
                        streams.len().try_into().unwrap_or(u32::MAX),
                    ),
                );

                // The wait moves from the fetch to the write it caused, so
                // the route is answered once the rows are actually there
                // rather than once Plaid has spoken.
                let writing = self.passing(*call_id);

                actions.push(Action::ToStore {
                    action: ActionStore::SaveFetched {
                        call_id: writing,
                        item_id: item_id.clone(),
                        institution_name: connection.as_ref().map(|c| c.name.clone()),
                        added: added.clone(),
                        modified: modified.clone(),
                        removed: removed.clone(),
                        accounts: connection
                            .as_ref()
                            .map(|c| c.accounts.clone())
                            .unwrap_or_default(),
                        streams: streams.clone(),
                        cursor: cursor.clone(),
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

                // Written down before the URL is handed over. The other order
                // leaves a window where somebody could finish a session we
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

                // A token is the answer whether or not the session says it has
                // finished: somebody who added one bank and is partway through
                // a second has produced a real Item already.
                //
                // The first one wins and the rest stay pending. One close
                // attaches one bank, because the route can only be told about
                // one — the next close collects the next.
                let exchangeable = polls
                    .iter()
                    .find_map(|poll| Some((&poll.link_token, poll.public_tokens.first()?)));

                if let Some((link_token, public_token)) = exchangeable {
                    // The same route is still waiting, one call further along.
                    actions.push(Action::ToSource {
                        action: ActionSource::ExchangePublicToken {
                            call_id: self.calling(request_id),
                            link_token: link_token.clone(),
                            public_token: public_token.clone(),
                        },
                    });

                    return;
                }

                // Finished with nothing: they closed the sheet. An outcome,
                // not a failure — and the session is marked done so it is not
                // polled again forever. One that has not finished is somebody
                // still typing their password, and is left pending so the next
                // close asks about it again.
                for poll in polls.iter().filter(|poll| poll.finished) {
                    actions.push(Action::ToStore {
                        action: ActionStore::CollectLink {
                            call_id: self.calling(request_id),
                            link_token: poll.link_token.clone(),
                        },
                    });
                }

                self.answer(request_id, ActionRouter::Linked { count: 0 }, actions);
            }

            EventSource::ItemLinked {
                call_id,
                link_token,
                item_id,
            } => {
                let Some(request_id) = self.answered(*call_id) else {
                    return;
                };

                actions.push(Action::ToStore {
                    action: ActionStore::SaveItem {
                        call_id: self.calling(request_id),
                        item_id: item_id.clone(),
                        link_token: link_token.clone(),
                    },
                });

                // Collected here only when a close is what asked for it. A
                // webhook arrives unprompted, so the session has to stay
                // standing until `/link/finish` reports it — a collected one
                // is a session that route can no longer see.
                if !self.collects_by_webhook() {
                    actions.push(Action::ToStore {
                        action: ActionStore::CollectLink {
                            call_id: self.calling(request_id),
                            link_token: link_token.clone(),
                        },
                    });
                }

                // Start reading the history now rather than waiting for the
                // next `/snapshot` to trigger it. Two reasons: the first sync
                // of a new bank is the long one and there is no sense making
                // somebody wait for it twice, and a screen that watches a sync
                // needs one to be running before it can watch anything.
                //
                // `call` rather than `calling`: nothing is parked on this.
                // The route below is answered as soon as the bank is attached,
                // which is the question it asked — whether the history has
                // arrived yet is a different one, and `/sync/status` is where
                // it gets asked. Registering a waiter here would hold
                // `/link/finish` open for the whole first sync, which is the
                // wait this is meant to remove.
                let call_id = self.call();

                // A bank attached seconds ago has no cursor. Passing one would
                // ask Plaid to resume a sync that never happened.
                self.fetch(call_id, item_id.clone(), None, actions);

                // Plaid's POST caused this, so the route that caused it is not
                // the route that wants the answer: it is owed nothing beyond
                // "taken", and what became of the session goes to
                // `/link/finish` — now, or when it asks.
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

                // An exchange that failed, on a session Plaid told us about.
                // Nothing came of it, which is an answer somebody is waiting
                // for — see [`Self::settled`].
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

                self.answer(request_id, ActionRouter::Linked { count: 0 }, actions);
            }

            EventSource::PlaidProgressed {
                item_id,
                stage,
                institution,
                accounts,
                transactions,
                streams,
            } => {
                self.syncing.insert(
                    item_id.clone(),
                    WireSyncStatus::new(
                        *stage,
                        institution.clone(),
                        *accounts,
                        *transactions,
                        *streams,
                    ),
                );
            }

            EventSource::PlaidFailed {
                call_id,
                item_id,
                code,
            } => {
                // Nothing arrived, so there is nothing to report but that it
                // is over. Dropped rather than marked done: a screen polling a
                // fetch that died would otherwise sit on its last stage.
                self.syncing.remove(item_id);

                if code == ITEM_LOGIN_REQUIRED {
                    actions.push(Action::ToProcess {
                        action: ActionProcess::ReportLoginRequired,
                    });

                    // Written down, or the server comes back up and asks Plaid
                    // the same question it was just refused.
                    actions.push(Action::ToStore {
                        action: ActionStore::SaveStatus {
                            call_id: self.passing(*call_id),
                            item_id: item_id.clone(),
                            error_code: Some(code.clone()),
                        },
                    });

                    return;
                }

                // Nothing to write, so a bank that failed has reported and the
                // route is not left hanging for one that will never answer.
                self.landed(*call_id, actions);
            }
        }
    }

    fn handle_event_process(&mut self, event: &EventProcess, actions: &mut Vec<Action>) {
        match event {
            // Up. Nothing is pulled into memory at boot: the store is read per
            // request, for the user who asked, which is what makes a second
            // server process possible. Entropy is what it is still waiting
            // for, and the seed behind this event is what brings it.
            EventProcess::Started { collect } => {
                self.collect = Some(*collect);

                if self.state == State::Stopped {
                    self.state = State::Starting;
                }
            }

            EventProcess::Seeded { seed } => {
                // A seed on the way out is dropped: nothing will be drawn from
                // it, and taking it would put a stopping machine back in
                // service.
                if self.state == State::Stopping {
                    return;
                }

                self.rng = Some(ChaCha20Rng::from_seed(*seed));

                // Reseeding is the same event as the first seed, and on a
                // machine already started it changes nothing but the
                // generator.
                if self.state == State::Starting {
                    self.state = State::Started;
                }
            }

            EventProcess::Tick { now } => {
                // Time only ever moves forward. A runtime handing in a stale
                // reading would make a freshness window wrong in a way nothing
                // else here would catch.
                debug_assert!(
                    *now >= self.now,
                    "time went backwards: {now} after {}",
                    self.now,
                );

                self.now = *now;

                // Callers who have stopped calling are forgotten here, or the
                // limiter's maps grow for the life of the process (#40). Free
                // when there is nothing to drop, and this is the one turn that
                // happens whether or not anything else did.
                self.requests.sweep(self.now);

                self.handle_timeouts(actions);
            }
        }
    }

    fn handle_event_store(&mut self, event: &EventStore, actions: &mut Vec<Action>) {
        match event {
            EventStore::WriteSuccess { call_id } => self.landed(*call_id, actions),

            // Reported rather than silent — the log is meant to be a recording
            // of the session, and "the store would not take it" is part of the
            // story — and then it ends the wait like a write that worked.
            // Holding a route for a write that failed would hold it forever.
            EventStore::WriteError { call_id, message } => {
                tracing::error!(message, "could not write the store");

                self.landed(*call_id, actions);
            }
        }
    }

    fn handle_event_terminal(&mut self, event: &EventTerminal, actions: &mut Vec<Action>) {
        match event {
            // Both mean stop, and they mean it the same way here — the
            // difference matters to whoever sent it, not to us.
            EventTerminal::SigInt | EventTerminal::SigTerm => {
                // Idempotent: a second ^C while the first is still draining
                // should not queue a second shutdown.
                if self.state == State::Stopping {
                    return;
                }

                self.state = State::Stopping;

                actions.push(Action::ToProcess {
                    action: ActionProcess::Shutdown,
                });
            }
        }
    }

    /// What a route is owed, sent the moment nothing it started is still out.
    ///
    /// Every answer goes through here rather than onto `actions` directly, so
    /// no route has to remember what it ought to wait for.
    fn answer(&mut self, request_id: RequestId, action: ActionRouter, actions: &mut Vec<Action>) {
        if self.outstanding(request_id) {
            self.held.insert(request_id, action);

            return;
        }

        actions.push(Action::ToRouter { request_id, action });
    }

    /// One call has landed. Release the answer it was the last thing holding.
    ///
    /// Silent when nothing was waiting: that is the ordinary shape of a sync
    /// nobody asked for, unlike [`answered`](Self::answered).
    fn landed(&mut self, call_id: CallId, actions: &mut Vec<Action>) {
        let Some(Waiting { request_id, .. }) = self.waiting.remove(&call_id) else {
            return;
        };

        // Somebody with three banks is answered after the third, not the first.
        if self.outstanding(request_id) {
            return;
        }

        let Some(action) = self.held.remove(&request_id) else {
            return;
        };

        actions.push(Action::ToRouter { request_id, action });
    }

    /// Go and read one Item, and remember that we are.
    ///
    /// The two happen together, and that is the whole point of the function.
    /// `syncing` used to fill in when the first stage *landed*, which left a
    /// gap: `/link/finish` answered, the phone asked how the sync was going,
    /// and the fetch had not started yet — so it was told nothing was running.
    /// "Nothing is running" and "it has finished" are the same answer, so the
    /// screen watching it decided the sync was over before it began and never
    /// asked again.
    ///
    /// Recording it here closes that, and closes it properly rather than by
    /// being quicker: this happens in the same turn as the action, so there is
    /// no interval in which a fetch has been asked for and nothing says so.
    fn fetch(
        &mut self,
        call_id: CallId,
        item_id: String,
        cursor: Option<String>,
        actions: &mut Vec<Action>,
    ) {
        // Nothing is known yet — not the counts and not even the bank's name.
        // What matters is that it is *not* finished.
        self.syncing.insert(
            item_id.clone(),
            WireSyncStatus::new(SyncStage::Accounts, None, 0, 0, 0),
        );

        actions.push(Action::ToSource {
            action: ActionSource::FetchPlaid {
                call_id,
                item_id,
                cursor,
            },
        });
    }

    /// One more bank attached on this request.
    ///
    /// Counted rather than set, because one `/link/finish` collects every
    /// session the caller has open and can attach two. Each arrives as its
    /// own `ItemLinked`, turns apart, and the route is owed one answer — so
    /// the count is carried on the answer being held rather than in a tally
    /// beside it that could be left behind when the request goes.
    fn linked_one(&mut self, request_id: RequestId, actions: &mut Vec<Action>) {
        let count = match self.held.get(&request_id) {
            Some(ActionRouter::Linked { count }) => *count,
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

    /// A fresh [`CallId`] that inherits whatever `call_id` was holding.
    ///
    /// One call causing the next is the ordinary shape here — a fetch is
    /// written, a poll is exchanged — and the route waits for the end of that
    /// chain, not for its first link.
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

    /// Who was waiting on this call, now that it has answered.
    ///
    /// Absent means an answer to a question we have no record of asking, which
    /// cannot happen — every call is minted here. Reported rather than
    /// guessed at: there is no route to send it to, and picking one would send
    /// somebody another person's bank.
    fn answered(&mut self, call_id: CallId) -> Option<RequestId> {
        let Some(Waiting { request_id, .. }) = self.waiting.remove(&call_id) else {
            tracing::error!(call_id, "a source answered a call nobody is waiting on");

            return None;
        };

        Some(request_id)
    }

    /// Stop waiting on calls that have taken too long.
    ///
    /// The only thing in the machine that happens because *nothing* happened,
    /// and the reason there is a tick at all. A source that goes quiet — a
    /// connection that hangs rather than fails — produces no event, so without
    /// this the route waiting on it is parked for as long as the process runs.
    ///
    /// What it answers with is whatever was already decided, and
    /// [`ActionRouter::Unavailable`] when nothing was. Serving what we have
    /// beats a request that never returns.
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

            // Another of this route's calls may still answer, and would then
            // release the answer itself.
            if self.outstanding(request_id) {
                continue;
            }

            let action = self
                .held
                .remove(&request_id)
                .unwrap_or(ActionRouter::Unavailable);

            actions.push(Action::ToRouter { request_id, action });
        }
    }

    /// Let go of the routes waiting on a Link session Plaid has not reported.
    ///
    /// The answer is `Pending` rather than "nothing came of it", because at
    /// this point neither is known: the bank may still be landing. The screen
    /// that follows a pending answer is the one that polls `/sync/status`, so
    /// a bank that arrives a moment later still shows up — which is the whole
    /// reason there is a third answer rather than a guess between the two.
    ///
    /// Outcomes nobody ever came back for go here too. They are written down
    /// for a route that has not called yet, and one that never calls would
    /// otherwise keep its entry for the life of the process.
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

        // One route parked on two sessions times out twice, and is owed one
        // answer either way.
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
    /// Pure, and reads the clock it was handed rather than one of its own —
    /// the whole reason this lives in a state machine.
    fn should_fetch(&mut self, item: &ItemFacts, force: bool) -> bool {
        // Nothing new starts once we are on the way out.
        if self.state == State::Stopping {
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

        // A fetch is three calls, and the strictest budget is the one that
        // decides. Asked last so that a request refused for staleness does not
        // spend a slot.
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

    pub fn shutting_down(&self) -> bool {
        self.state == State::Stopping
    }
}

/// Where a stage sits in the order they run in.
///
/// `server-wire` owns the order itself; this is only how to compare two of
/// them, which is what picking the furthest-behind fetch needs.
fn stage_index(stage: SyncStage) -> usize {
    SyncStage::ALL
        .iter()
        .position(|s| *s == stage)
        .unwrap_or_default()
}
