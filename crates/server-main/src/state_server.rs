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
use crate::Event;
use crate::EventProcess;
use crate::EventRouter;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;
use crate::ItemFacts;
use crate::RequestId;
use crate::login_code;
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

/// Where the server is in its own life.
///
/// The two things the machine cannot do in every state are minting credentials
/// and starting work, and each belongs to a state rather than to a flag beside
/// one: the generator lives in [`Phase::Running`], so having one is the same
/// fact as being in it, and there is no state in which asking for it can fail.
#[derive(Debug, Default)]
enum Phase {
    /// Up, with no entropy yet.
    #[default]
    Starting,

    /// Seeded, and serving.
    ///
    /// The generator is ChaCha20 rather than a fast PRNG: these are
    /// credentials, and a generator whose state can be recovered from a couple
    /// of outputs would hand an attacker every code after the ones they saw.
    /// It is boxed because it is two orders of magnitude larger than the other
    /// variants.
    Running { rng: Box<ChaCha20Rng> },

    /// A signal has arrived. What is outstanding finishes; nothing new starts,
    /// and nothing new is minted.
    Draining,
}

#[derive(Debug, Default)]
pub struct StateServer {
    phase: Phase,

    /// What Plaid will let us send. The one thing here that genuinely cannot
    /// be read back from the store — it is a fact about this process's last
    /// minute, and it is why "may I call?" is arithmetic in the machine rather
    /// than a guess in the runtime.
    limiter: PlaidRateLimiter,

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
            Event::FromRouter { request_id, event } => {
                self.handle_event_router(*request_id, event, actions)
            }
            Event::FromSource { event } => self.handle_event_source(event, actions),
            Event::FromStore { event } => self.handle_event_store(event, actions),
            Event::FromProcess { event } => self.handle_event_process(event, actions),
            Event::FromTerminal { event } => self.handle_event_terminal(event, actions),
        }
    }

    fn handle_event_router(
        &mut self,
        request_id: RequestId,
        event: &EventRouter,
        actions: &mut Vec<Action>,
    ) {
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

            EventRouter::LinkClosed { pending } => {
                // Nothing open. Somebody closed a sheet we have no record of,
                // or collected this one already — either way there is nothing
                // to go and ask about.
                if pending.is_empty() {
                    self.answer(request_id, ActionRouter::NotLinked, actions);

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
                let Phase::Running { rng } = &mut self.phase else {
                    tracing::error!("asked for a sign-in code with no generator to draw it from");

                    self.answer(request_id, ActionRouter::Unavailable, actions);

                    return;
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

                let Phase::Running { rng } = &mut self.phase else {
                    tracing::error!("a code was accepted with no generator to draw a token from");

                    self.answer(request_id, ActionRouter::Rejected, actions);

                    return;
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

            EventRouter::SnapshotRequested { force, items } => {
                let asked: Vec<_> = items
                    .iter()
                    .filter(|item| self.should_fetch(item, *force))
                    .map(|item| (item.item_id.clone(), item.cursor.clone()))
                    .collect();

                for (item_id, cursor) in asked {
                    actions.push(Action::ToSource {
                        action: ActionSource::FetchPlaid {
                            call_id: self.calling(request_id),
                            item_id,
                            cursor,
                        },
                    });
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

                self.answer(request_id, ActionRouter::NotLinked, actions);
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

                actions.push(Action::ToStore {
                    action: ActionStore::CollectLink {
                        call_id: self.calling(request_id),
                        link_token: link_token.clone(),
                    },
                });

                self.answer(request_id, ActionRouter::Linked, actions);
            }

            EventSource::LinkAbandoned {
                call_id,
                link_token,
            } => {
                let Some(request_id) = self.answered(*call_id) else {
                    return;
                };

                actions.push(Action::ToStore {
                    action: ActionStore::CollectLink {
                        call_id: self.calling(request_id),
                        link_token: link_token.clone(),
                    },
                });

                self.answer(request_id, ActionRouter::NotLinked, actions);
            }

            EventSource::PlaidFailed {
                call_id,
                item_id,
                code,
            } => {
                if code == "ITEM_LOGIN_REQUIRED" {
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
            // Nothing to do. The store is read per request, for the user who
            // asked, so there is nothing to pull into memory at boot — which
            // is what makes a second server process possible.
            EventProcess::Started => {}

            EventProcess::Seeded { seed } => match self.phase {
                // Reseeding replaces the generator; a seed arriving on the way
                // out is dropped, because nothing will be drawn from it.
                Phase::Starting | Phase::Running { .. } => {
                    self.phase = Phase::Running {
                        rng: Box::new(ChaCha20Rng::from_seed(*seed)),
                    };
                }

                Phase::Draining => {}
            },

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
                if matches!(self.phase, Phase::Draining) {
                    return;
                }

                self.phase = Phase::Draining;

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

    /// Whether to go back to Plaid about this bank.
    ///
    /// Pure, and reads the clock it was handed rather than one of its own —
    /// the whole reason this lives in a state machine.
    fn should_fetch(&mut self, item: &ItemFacts, force: bool) -> bool {
        // Nothing new starts once we are on the way out.
        if matches!(self.phase, Phase::Draining) {
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
        matches!(self.phase, Phase::Draining)
    }
}
