//! Signing up: the code, the session it buys, and the bank sign-in that
//! follows.
//!
//! Everything that gets somebody from an empty screen to a connected bank, and
//! nothing that happens afterwards. It holds what that takes and decides what
//! its own events mean. [`StateServer`](crate::StateServer) routes them here;
//! `handle_events` is still the only way anything changes.
//!
//! The hard part is the bank sign-in, and it is hard because two things race
//! with no ordering between them: the redirect that closes Plaid's sheet, and
//! Plaid's POST saying what the session came to. Either can arrive first, so
//! whichever it is waits for the other.
//!
//! The routes in flight live here too, one entry each at the step it has
//! reached ([`Pending`]), and the calls they are waiting on, one entry each
//! ([`Waiting`]). Nearly all of them are this machine's — a code to save, a
//! session to read, a token to exchange — and a handler that could not start
//! a call or send an answer would have to hand both back to its parent to do.
//! The few that are not signing up (a snapshot, a sync, a webhook) draw their
//! ids from the same counter, so an event log still replays to the same
//! numbers.

use std::collections::HashMap;

use api_common::Timestamp;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use uuid::Uuid;

use crate::Action;
use crate::ActionEmail;
use crate::ActionRouter;
use crate::ActionSource;
use crate::ActionStore;
use crate::Authenticated;
use crate::CallId;
use crate::LinkOutcome;
use crate::LinkPoll;
use crate::LinkSession;
use crate::Owed;
use crate::Pending;
use crate::PlaidCollect;
use crate::RequestId;
use crate::StoredCode;
use crate::Waiting;
use crate::email_address;
use crate::login_code;
use crate::session_token;

/// How long a session lasts. Long, because the alternative is asking somebody
/// to check their email to look at their own balance.
pub const SESSION_GOOD_FOR_SECS: u64 = 90 * 24 * 60 * 60;

/// How often the expired sessions are swept out of the store. Nothing depends
/// on it being prompt — an expired session signs nobody in either way — so
/// this is only about the table not growing.
pub const SWEEP_SESSIONS_EVERY_SECS: u64 = 60 * 60;

/// How long `/link/finish` waits for Plaid to say what a session came to.
///
/// The redirect that closes the sheet and Plaid's POST have no ordering
/// between them, so the route waits.
pub const LINK_ANSWER_GOOD_FOR_SECS: u64 = 10;

/// How long an outcome nobody has asked about is kept. A phone that has not
/// asked within ten minutes is not going to: the sheet is long closed.
pub const OUTCOME_GOOD_FOR_SECS: u64 = 10 * 60;

/// How long to wait on a source before giving up on it. Not a latency budget
/// but the point past which a call has almost certainly gone quiet.
pub const CALL_GOOD_FOR_SECS: u64 = 30;

/// Signing up, from the first code to the first bank.
#[derive(Debug, Default)]
pub struct StateSignup {
    /// Where sign-in codes and session tokens come from, seeded by
    /// [`EventProcess::Seeded`](crate::EventProcess).
    ///
    /// ChaCha20 rather than a fast PRNG: a generator whose state can be
    /// recovered from its outputs hands an attacker every code after them.
    rng: Option<ChaCha20Rng>,

    /// When the store's expired sessions were last swept. The first tick
    /// starts the clock rather than sweeping, so a restart brings none on.
    swept: Option<Timestamp>,

    /// Which channel collects the result of a Link session, read out of the
    /// environment on [`EventProcess::Started`](crate::EventProcess). `None`
    /// is a machine that has not been told the process is up.
    collect: Option<PlaidCollect>,

    /// Each Link session with one half of its race in, keyed by the
    /// `link_token` that `SESSION_FINISHED` carries. Always empty under
    /// `PLAID_COLLECT=poll`, where the close causes the collection and nothing
    /// races.
    sessions: HashMap<String, LinkSession>,

    /// The next [`CallId`] to hand out. A counter rather than anything drawn,
    /// so the same log replays to the same numbers.
    next_call: CallId,

    /// Which route, if any, is parked on the answer to each call in flight.
    /// An absent entry is a call nobody is waiting on — a sync on a timer.
    waiting: HashMap<CallId, Waiting>,

    /// Every route in flight, at the step it has reached. Entered on the turn
    /// a request arrives and left on the turn it is answered, so what a
    /// request was doing outlives the turn it arrived in.
    requests: HashMap<RequestId, Pending>,
}

impl StateSignup {
    /// What the process came up with.
    pub fn started(&mut self, collect: PlaidCollect) {
        self.collect = Some(collect);
    }

    /// Take the entropy the process drew.
    pub fn seed(&mut self, seed: &[u8; 32]) {
        self.rng = Some(ChaCha20Rng::from_seed(*seed));
    }

    /// Mint nothing more. The generator goes rather than a flag beside it: a
    /// credential drawn by a process that is about to exit is one nobody can
    /// use, and there is no way back from here.
    pub fn stopping(&mut self) {
        self.rng = None;
    }

    /// Somebody asked for a code.
    pub fn code_requested(
        &mut self,
        request_id: RequestId,
        email: &str,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // Answered as though it had been taken: the route says the same thing
        // whether or not an address has an account, and the shape of the
        // string must not say otherwise.
        if !email_address::plausible(email) {
            tracing::warn!("a sign-in was asked for with something that is not an address",);

            self.answer(request_id, ActionRouter::Accepted, now, actions);

            return;
        }

        let Some(rng) = self.rng.as_mut() else {
            tracing::error!("asked for a sign-in code by a machine that mints none");

            self.answer(request_id, ActionRouter::Unavailable, now, actions);

            return;
        };

        let code = login_code::generate(rng);

        let expires_at = Timestamp::from_secs(now.as_secs() + login_code::GOOD_FOR.as_secs());

        // Two actions because they go to different places, and only one of
        // them may ever hold the plaintext.
        actions.push(Action::ToStore {
            action: ActionStore::SaveCode {
                call_id: self.calling(request_id, now),
                email: email.to_string(),
                code_sha256: login_code::hash(&code),
                expires_at,
            },
        });

        actions.push(Action::ToEmail {
            action: ActionEmail::SendCode {
                email: email.to_string(),
                code,
            },
        });

        self.answer(request_id, ActionRouter::Accepted, now, actions);
    }

    /// Somebody typed a code. What they typed is kept, because the store has
    /// to be asked what it holds against the address first.
    pub fn code_submitted(
        &mut self,
        request_id: RequestId,
        email: &str,
        code: &str,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        self.waits_on(
            request_id,
            Pending::Submitted {
                email: email.to_string(),
                code: code.to_string(),
            },
        );

        actions.push(Action::ToStore {
            action: ActionStore::LoadCode {
                call_id: self.calling(request_id, now),
                email: email.to_string(),
            },
        });
    }

    /// Check a code against what the store held, and sign somebody in or not.
    /// A turn later than the submission, which is why the machine kept what
    /// they typed.
    pub fn code_loaded(
        &mut self,
        request_id: RequestId,
        email: &str,
        code: &str,
        stored: Option<&StoredCode>,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // No code standing against this address, and a rejection like any
        // other: telling them apart says which addresses have one in flight.
        let Some(stored) = stored else {
            self.answer(request_id, ActionRouter::Rejected, now, actions);

            return;
        };

        let verdict = login_code::verify(stored, code, now);

        if verdict != login_code::Verdict::Accepted {
            // Counted whatever the reason, expiry included: resetting the cap
            // by waiting would give unlimited guesses at the next code.
            actions.push(Action::ToStore {
                action: ActionStore::RecordAttempt {
                    call_id: self.calling(request_id, now),
                    email: email.to_string(),
                },
            });

            tracing::warn!(email, ?verdict, "a sign-in code was rejected");

            self.answer(request_id, ActionRouter::Rejected, now, actions);

            return;
        }

        let Some(rng) = self.rng.as_mut() else {
            tracing::error!("a code was accepted by a machine that mints none");

            self.answer(request_id, ActionRouter::Rejected, now, actions);

            return;
        };

        let token = login_code::generate_token(rng);

        actions.push(Action::ToStore {
            action: ActionStore::CreateSession {
                call_id: self.calling(request_id, now),
                email: email.to_string(),
                token_sha256: session_token::hash(&token),
                expires_at: Timestamp::from_secs(now.as_secs() + SESSION_GOOD_FOR_SECS),
            },
        });

        // Single use, and spent only once it has bought something.
        actions.push(Action::ToStore {
            action: ActionStore::DeleteCode {
                call_id: self.calling(request_id, now),
                email: email.to_string(),
            },
        });

        self.answer(request_id, ActionRouter::SignedIn { token }, now, actions);
    }

    /// Somebody wants a bank sign-in opened. Nothing to read: their id is the
    /// whole of what it takes.
    pub fn link_requested(
        &mut self,
        request_id: RequestId,
        user_id: Uuid,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        self.waits_on(request_id, Pending::Opening);

        actions.push(Action::ToSource {
            action: ActionSource::OpenLink {
                call_id: self.calling(request_id, now),
                user_id,
            },
        });
    }

    /// A sheet closed. Which sessions this caller has open decides what that
    /// means, so the store is asked first.
    pub fn link_closing(
        &mut self,
        request_id: RequestId,
        user_id: Uuid,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        self.waits_on(
            request_id,
            Pending::Loading {
                user_id,
                event: Authenticated::LinkClosed,
            },
        );

        actions.push(Action::ToStore {
            action: ActionStore::LoadPendingLinks {
                call_id: self.calling(request_id, now),
                user_id,
            },
        });
    }

    /// What a sheet closing came to, now that the open sessions are known.
    pub fn pending_links_loaded(
        &mut self,
        request_id: RequestId,
        link_tokens: &[String],
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // Collection already happens unprompted, so this route asks rather
        // than triggers: Plaid would charge a call for a token already gone.
        if self.collects_by_webhook() {
            self.closed_by_webhook(request_id, link_tokens, now, actions);

            return;
        }

        // Nothing open, so nothing to go and ask about.
        if link_tokens.is_empty() {
            self.answer(request_id, ActionRouter::LinkAbandoned, now, actions);

            return;
        }

        self.waits_on(request_id, Pending::Polling);

        actions.push(Action::ToSource {
            action: ActionSource::PollLinks {
                call_id: self.calling(request_id, now),
                link_tokens: link_tokens.to_vec(),
            },
        });
    }

    /// A sheet closed, on a server Plaid calls.
    ///
    /// The redirect that closed it says nothing — Plaid's documentation is
    /// explicit that reaching the completion URI is not a connection, and that
    /// the answer is `SESSION_FINISHED` — so this route only asks.
    fn closed_by_webhook(
        &mut self,
        request_id: RequestId,
        pending: &[String],
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // Looked for before anybody is parked, because the POST usually wins
        // the race and waiting first would make it sit until it timed out.
        if let Some((link_token, outcome)) = self.collected(pending) {
            self.report(request_id, &link_token, outcome, now, actions);

            return;
        }

        // Nothing open and nothing recorded: parking would promise nothing.
        if pending.is_empty() {
            self.answer(request_id, ActionRouter::LinkAbandoned, now, actions);

            return;
        }

        // Parked on every session the caller has open. The first to report
        // answers, and the timeout is what stops this being forever.
        for link_token in pending {
            let displaced = self.sessions.insert(
                link_token.clone(),
                LinkSession::Watched {
                    request_id,
                    since: now,
                },
            );

            // Two calls about the same session: the older is let go rather
            // than parked on a key it does not own, which nothing answers.
            if let Some(LinkSession::Watched {
                request_id: displaced,
                ..
            }) = displaced
                && displaced != request_id
            {
                self.answer(displaced, ActionRouter::LinkPending, now, actions);
            }
        }

        self.waits_on(request_id, Pending::Watching);
    }

    /// The recorded outcome for one of these sessions, taken as it is read. A
    /// bank attached wins over one that was not: somebody who backed out of
    /// their first bank and connected a second has two sessions.
    fn collected(&mut self, pending: &[String]) -> Option<(String, LinkOutcome)> {
        let found = pending
            .iter()
            .filter_map(|link_token| match self.sessions.get(link_token) {
                Some(LinkSession::Settled { outcome, .. }) => Some((link_token, *outcome)),
                _ => None,
            })
            .max_by_key(|(_, outcome)| *outcome == LinkOutcome::Linked)
            .map(|(link_token, outcome)| (link_token.clone(), outcome))?;

        self.sessions.remove(&found.0);

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
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // An entry left behind sends the next settlement to a dead request.
        self.unwatch(request_id);

        actions.push(Action::ToStore {
            action: ActionStore::CollectLink {
                call_id: self.calling(request_id, now),
                link_token: link_token.to_string(),
            },
        });

        self.answer(request_id, outcome.answer(), now, actions);
    }

    /// One Link session has come to something, on a server Plaid calls.
    /// Either a route is parked on it or one will ask, and this is the one
    /// place that does not care which.
    fn settled(
        &mut self,
        link_token: &str,
        outcome: LinkOutcome,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // No token means `/link/start` failed before there was one.
        if link_token.is_empty() {
            return;
        }

        let Some(LinkSession::Watched { request_id, .. }) = self.sessions.remove(link_token) else {
            self.sessions.insert(
                link_token.to_string(),
                LinkSession::Settled { outcome, at: now },
            );

            return;
        };

        self.report(request_id, link_token, outcome, now, actions);
    }

    /// Whether Plaid calls us with the result of a Link session.
    fn collects_by_webhook(&self) -> bool {
        self.collect == Some(PlaidCollect::Webhook)
    }

    /// Plaid has a page for somebody to sign in to their bank on.
    pub fn link_opened(
        &mut self,
        request_id: RequestId,
        user_id: Uuid,
        link_token: &str,
        hosted_link_url: &str,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // Written down first, or somebody could finish a session we have no
        // record of asking for.
        actions.push(Action::ToStore {
            action: ActionStore::SaveLink {
                call_id: self.calling(request_id, now),
                link_token: link_token.to_string(),
                user_id,
            },
        });

        self.answer(
            request_id,
            ActionRouter::LinkOpened {
                url: hosted_link_url.to_string(),
            },
            now,
            actions,
        );
    }

    /// What Plaid says about the sessions this caller had open, on a server
    /// Plaid does not call.
    pub fn links_polled(
        &mut self,
        request_id: RequestId,
        polls: &[LinkPoll],
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // A token is the answer whether or not the session says it has
        // finished. The first wins and the rest wait for the next close,
        // because the route can be told about one.
        let exchangeable = polls
            .iter()
            .find_map(|poll| Some((poll.link_token.clone(), poll.public_tokens.first()?.clone())));

        if let Some((link_token, public_token)) = exchangeable {
            self.waits_on(request_id, Pending::Exchanging);

            actions.push(Action::ToSource {
                action: ActionSource::ExchangePublicToken {
                    call_id: self.calling(request_id, now),
                    link_token,
                    public_token,
                },
            });

            return;
        }

        // They closed the sheet, and it is marked done so it is not polled
        // forever. One still running is somebody typing.
        let finished: Vec<String> = polls
            .iter()
            .filter(|poll| poll.finished)
            .map(|poll| poll.link_token.clone())
            .collect();

        for link_token in finished {
            actions.push(Action::ToStore {
                action: ActionStore::CollectLink {
                    call_id: self.calling(request_id, now),
                    link_token,
                },
            });
        }

        self.answer(request_id, ActionRouter::LinkAbandoned, now, actions);
    }

    /// A bank is attached. The first sync follows this write rather than
    /// racing it: whose bank it is comes from the Link session, and the write
    /// is what resolves that.
    pub fn item_linked(
        &mut self,
        request_id: RequestId,
        link_token: &str,
        item_id: &str,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        actions.push(Action::ToStore {
            action: ActionStore::SaveItem {
                call_id: self.calling(request_id, now),
                item_id: item_id.to_string(),
                link_token: link_token.to_string(),
            },
        });

        // A webhook arrives unprompted, so the session has to stand until
        // `/link/finish` reports it.
        if !self.collects_by_webhook() {
            actions.push(Action::ToStore {
                action: ActionStore::CollectLink {
                    call_id: self.calling(request_id, now),
                    link_token: link_token.to_string(),
                },
            });
        }

        // Plaid's POST caused this and is owed nothing beyond "taken": the
        // outcome goes to `/link/finish`.
        if self.collects_by_webhook() {
            self.answer(request_id, ActionRouter::Accepted, now, actions);

            self.settled(link_token, LinkOutcome::Linked, now, actions);

            return;
        }

        self.linked_one(request_id, now, actions);
    }

    /// An exchange came to nothing.
    pub fn link_abandoned(
        &mut self,
        request_id: RequestId,
        link_token: &str,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // An exchange that failed, on a session Plaid told us about: nothing
        // came of it, and somebody is waiting to hear that.
        if self.collects_by_webhook() && !link_token.is_empty() {
            self.answer(request_id, ActionRouter::Accepted, now, actions);

            self.settled(link_token, LinkOutcome::Nothing, now, actions);

            return;
        }

        actions.push(Action::ToStore {
            action: ActionStore::CollectLink {
                call_id: self.calling(request_id, now),
                link_token: link_token.to_string(),
            },
        });

        self.answer(request_id, ActionRouter::LinkAbandoned, now, actions);
    }

    /// Plaid says a Link session is over, and hands us what it produced.
    pub fn session_finished(
        &mut self,
        request_id: RequestId,
        link_token: &str,
        public_tokens: &[String],
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // Finished with nothing: they closed the sheet. An outcome Plaid
        // *said*, not one inferred from an empty poll.
        if public_tokens.is_empty() {
            self.answer(request_id, ActionRouter::Accepted, now, actions);

            self.settled(link_token, LinkOutcome::Nothing, now, actions);

            return;
        }

        // Every token, because Plaid says this once: one nobody exchanges is a
        // bank somebody connected and does not get.
        for public_token in public_tokens {
            actions.push(Action::ToSource {
                action: ActionSource::ExchangePublicToken {
                    call_id: self.calling(request_id, now),
                    link_token: link_token.to_string(),
                    public_token: public_token.clone(),
                },
            });
        }

        // Held until the exchanges have landed and been written, so a failure
        // mid-exchange is a webhook Plaid sends again.
        self.answer(request_id, ActionRouter::Accepted, now, actions);
    }

    /// One more bank attached on this request. Counted rather than set,
    /// because one `/link/finish` can attach two banks, turns apart; the count
    /// rides on the held answer rather than a tally that outlives it.
    fn linked_one(&mut self, request_id: RequestId, now: Timestamp, actions: &mut Vec<Action>) {
        let count = match self.requests.get(&request_id) {
            Some(Pending::Owed(Owed::Answer(ActionRouter::Linked { count }))) => *count,
            _ => 0,
        };

        self.answer(
            request_id,
            ActionRouter::Linked { count: count + 1 },
            now,
            actions,
        );
    }

    /// Let go of the routes waiting on a Link session Plaid has not reported,
    /// and drop the outcomes nobody ever came back for.
    ///
    /// `Pending` rather than "nothing came of it", because neither is known
    /// yet: the screen that follows it polls `/sync/status`, so a bank landing
    /// a moment later still shows up.
    pub fn link_timed_out(&mut self, now: Timestamp, actions: &mut Vec<Action>) {
        self.sessions.retain(|_, session| match session {
            LinkSession::Settled { at, .. } => now.since(*at).as_secs() < OUTCOME_GOOD_FOR_SECS,
            LinkSession::Watched { .. } => true,
        });

        let mut waited_out: Vec<RequestId> = self
            .sessions
            .values()
            .filter_map(|session| match session {
                LinkSession::Watched { request_id, since }
                    if now.since(*since).as_secs() >= LINK_ANSWER_GOOD_FOR_SECS =>
                {
                    Some(*request_id)
                }
                _ => None,
            })
            .collect();

        // One route parked on two sessions times out twice, owed one answer.
        waited_out.sort_unstable();
        waited_out.dedup();

        for request_id in waited_out {
            self.unwatch(request_id);

            tracing::info!(
                request_id,
                "plaid has not said what a link session came to yet",
            );

            self.answer(request_id, ActionRouter::LinkPending, now, actions);
        }
    }

    /// Ask the store to drop the sessions that have expired, about hourly.
    /// Decided here rather than on a runtime timer, so a log replays it.
    pub fn sweep_sessions(&mut self, now: Timestamp, actions: &mut Vec<Action>) {
        // Nothing is owed a sweep at boot: expired rows are already inert.
        let Some(swept) = self.swept else {
            self.swept = Some(now);

            return;
        };

        if now.since(swept).as_secs() < SWEEP_SESSIONS_EVERY_SECS {
            return;
        }

        self.swept = Some(now);

        actions.push(Action::ToStore {
            action: ActionStore::SweepSessions {
                call_id: self.call(),
                now,
            },
        });
    }

    /// A fresh [`CallId`], noted as owed to the route that is waiting.
    pub fn calling(&mut self, request_id: RequestId, now: Timestamp) -> CallId {
        let call_id = self.call();

        self.waiting.insert(
            call_id,
            Waiting {
                request_id,
                since: now,
            },
        );

        call_id
    }

    /// A fresh [`CallId`] for a call nobody is waiting on.
    pub fn call(&mut self) -> CallId {
        self.next_call += 1;

        self.next_call
    }

    /// A fresh [`CallId`] owed to the same route as `call_id`, which keeps its
    /// own wait. For an answer that causes two: this is the branch that does
    /// not carry the chain on, and the other takes the wait with
    /// [`passing`](Self::passing).
    pub fn alongside(&mut self, call_id: CallId, now: Timestamp) -> CallId {
        match self.waiting.get(&call_id) {
            Some(waiter) => {
                let request_id = waiter.request_id;

                self.calling(request_id, now)
            }
            None => self.call(),
        }
    }

    /// A fresh [`CallId`] that inherits whatever `call_id` was holding. One
    /// call causes the next, and the route waits for the end of that chain.
    pub fn passing(&mut self, call_id: CallId, now: Timestamp) -> CallId {
        match self.waiting.remove(&call_id) {
            Some(waiter) => self.calling(waiter.request_id, now),
            None => self.call(),
        }
    }

    /// Who was waiting on this call, now that it has answered. Absent means an
    /// answer to a question we have no record of asking, and picking a route
    /// for it would send somebody another person's bank.
    pub fn answered(&mut self, call_id: CallId) -> Option<RequestId> {
        let Some(Waiting { request_id, .. }) = self.waiting.remove(&call_id) else {
            tracing::error!(call_id, "a source answered a call nobody is waiting on");

            return None;
        };

        Some(request_id)
    }

    /// Let a route go from every session it is parked on.
    fn unwatch(&mut self, request_id: RequestId) {
        self.sessions.retain(|_, session| {
            !matches!(session, LinkSession::Watched { request_id: parked, .. } if *parked == request_id)
        });
    }

    /// Note the step a request has reached, replacing the one it was at.
    pub fn waits_on(&mut self, request_id: RequestId, step: Pending) {
        self.requests.insert(request_id, step);
    }

    /// The step a request was at, now that what it was waiting for has
    /// arrived. Absent is a route that was already answered and let go of.
    pub fn resumed(&mut self, request_id: RequestId) -> Option<Pending> {
        self.requests.remove(&request_id)
    }

    /// What a route is owed, sent the moment nothing it started is still out.
    /// Every answer goes through here, so no handler has to remember to wait.
    pub fn answer(
        &mut self,
        request_id: RequestId,
        action: ActionRouter,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        self.owe(request_id, Owed::Answer(action), now, actions);
    }

    /// Hold what a request is owed until everything it started has landed.
    ///
    /// Whatever step the route was at is over: deciding is the last one.
    pub fn owe(
        &mut self,
        request_id: RequestId,
        owed: Owed,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        if self.outstanding(request_id) {
            self.requests.insert(request_id, Pending::Owed(owed));

            return;
        }

        self.requests.remove(&request_id);

        self.release(request_id, owed, now, actions);
    }

    /// Nothing is outstanding, so this is what the request gets. A snapshot is
    /// one more call rather than an answer: the rows it reads are the ones the
    /// calls that just landed wrote.
    fn release(
        &mut self,
        request_id: RequestId,
        owed: Owed,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        match owed {
            Owed::Answer(action) => actions.push(Action::ToRouter { request_id, action }),

            Owed::Snapshot { user_id } => {
                self.waits_on(request_id, Pending::Reading);

                actions.push(Action::ToStore {
                    action: ActionStore::LoadSnapshot {
                        call_id: self.calling(request_id, now),
                        user_id,
                    },
                });
            }
        }
    }

    /// One call has landed. Release the answer it was the last thing holding.
    /// Silent when nothing was waiting, unlike [`answered`](Self::answered):
    /// that is the ordinary shape of a sync nobody asked for.
    pub fn landed(&mut self, call_id: CallId, now: Timestamp, actions: &mut Vec<Action>) {
        let Some(Waiting { request_id, .. }) = self.waiting.remove(&call_id) else {
            return;
        };

        // Somebody with three banks is answered after the third, not the first.
        if self.outstanding(request_id) {
            return;
        }

        let Some(owed) = self.decided(request_id) else {
            return;
        };

        self.release(request_id, owed, now, actions);
    }

    /// Take what a request decided, if it has. A request at any other step is
    /// left there: what it is waiting for is not a call landing.
    fn decided(&mut self, request_id: RequestId) -> Option<Owed> {
        if !matches!(self.requests.get(&request_id), Some(Pending::Owed(_))) {
            return None;
        }

        match self.requests.remove(&request_id) {
            Some(Pending::Owed(owed)) => Some(owed),
            _ => unreachable!("looked at on the line above"),
        }
    }

    /// Whether this route started anything that has not come back.
    fn outstanding(&self, request_id: RequestId) -> bool {
        self.waiting
            .values()
            .any(|waiter| waiter.request_id == request_id)
    }

    /// Stop waiting on calls that have taken too long.
    ///
    /// The reason there is a tick at all: a source that hangs rather than fails
    /// produces no event, and the route waiting on it would be parked forever.
    /// It answers with whatever was decided, and
    /// [`ActionRouter::Unavailable`] when nothing was.
    pub fn given_up(&mut self, now: Timestamp, actions: &mut Vec<Action>) {
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

            // Another of this route's calls may still answer and release it.
            if self.outstanding(request_id) {
                continue;
            }

            // Whatever step the route was at goes with the call it was
            // waiting on: giving up is deciding there will not be an answer.
            // A snapshot still reads and still serves — a call that never
            // answered costs the fresher rows, not the ledger already there.
            let owed = match self.requests.remove(&request_id) {
                Some(Pending::Owed(owed)) => owed,
                _ => Owed::Answer(ActionRouter::Unavailable),
            };

            self.release(request_id, owed, now, actions);
        }
    }

    /// Whether nothing is out and no route is in flight. The machine cannot
    /// come to a stop before this is true.
    pub fn quiet(&self) -> bool {
        self.waiting.is_empty() && self.requests.is_empty()
    }

    /// How many calls are out. See [`CensusServer`](crate::CensusServer).
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    /// How many routes are in flight.
    pub fn requests(&self) -> usize {
        self.requests.len()
    }

    /// How many Link sessions have one half of their race in.
    pub fn sessions(&self) -> usize {
        self.sessions.len()
    }
}
