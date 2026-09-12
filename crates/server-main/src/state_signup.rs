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

use std::collections::BTreeMap;

use api_common::Timestamp;
use api_common::ratelimit::RateLimiter;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use uuid::Uuid;

use src_plaid::PlaidEndpoint;
use src_plaid::PlaidRateCtx;
use src_plaid::PlaidRateLimiter;

use crate::Action;
use crate::ActionEmail;
use crate::ActionRouter;
use crate::ActionSourcePlaid;
use crate::ActionStore;
use crate::Attaching;
use crate::Authenticated;
use crate::CallId;
use crate::LinkOpener;
use crate::LinkOutcome;
use crate::LinkPoll;
use crate::LinkSession;
use crate::Owed;
use crate::Pending;
use crate::PlaidCollect;
use crate::RequestId;
use crate::StoredCode;
use crate::Waiting;
use crate::WebOrigins;
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

/// How many times a newly linked bank's write is asked for before the machine
/// stops asking.
///
/// More than one because the exchange in front of it spent an Item slot that
/// cannot be spent again, and a store that dropped a connection is the
/// ordinary reason the first attempt failed. Not many more because the route
/// is holding all the while: a store that has refused the same write five
/// times is down rather than busy.
pub const ATTACH_ATTEMPTS: u32 = 5;

/// How long to wait on a source before giving up on it. Not a latency budget
/// but the point past which a call has almost certainly gone quiet.
pub const CALL_GOOD_FOR_SECS: u64 = 30;

/// How long an exchange is kept out after the route that asked for it has been
/// let go of.
///
/// Far longer than [`CALL_GOOD_FOR_SECS`], because giving up on this call is
/// not giving up on a request. A slow or lost response says nothing about
/// whether Plaid created the Item, and the answer carries the only copy of the
/// access token there will ever be: forgetting it early is how a slot is spent
/// on a bank nothing can reach. It has a ceiling at all so that a Plaid that
/// never answers is not a process that never exits.
pub const EXCHANGE_GOOD_FOR_SECS: u64 = 5 * 60;

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

    /// Where the web app is served from, read out of the environment on
    /// [`EventProcess::Started`](crate::EventProcess). `None` is a machine
    /// that has not been told, and sends every bank sign-in back to the app.
    web_origins: Option<WebOrigins>,

    /// Each Link session with one half of its race in, keyed by the
    /// `link_token` that `SESSION_FINISHED` carries. Always empty under
    /// `PLAID_COLLECT=poll`, where the close causes the collection and nothing
    /// races.
    sessions: BTreeMap<String, LinkSession>,

    /// The next [`CallId`] to hand out. A counter rather than anything drawn,
    /// so the same log replays to the same numbers.
    next_call: CallId,

    /// Which route, if any, is parked on the answer to each call in flight.
    /// An absent entry is a call nobody is waiting on — a sync on a timer.
    waiting: BTreeMap<CallId, Waiting>,

    /// Every route in flight, at the step it has reached. Entered on the turn
    /// a request arrives and left on the turn it is answered, so what a
    /// request was doing outlives the turn it arrived in.
    requests: BTreeMap<RequestId, Pending>,

    /// The exchanges out, and when each went out.
    ///
    /// Kept apart from the wait because the two have different lifetimes: the
    /// route asking can be let go of and this call cannot. Every entry is an
    /// Item slot that may already be spent, and an access token nothing else
    /// holds a copy of.
    exchanging: BTreeMap<CallId, Timestamp>,

    /// Each bank whose write is in flight, keyed by the call it is out on.
    /// Its own table rather than a [`Pending`] step, because the route that
    /// caused it is not always the one waiting on it: a webhook attaches a
    /// bank that `/link/finish` is the one to hear about.
    attaching: BTreeMap<CallId, Attaching>,
}

impl StateSignup {
    /// What the process came up with.
    pub fn started(&mut self, collect: PlaidCollect, web_origins: WebOrigins) {
        self.collect = Some(collect);
        self.web_origins = Some(web_origins);
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

        // Who may have an account at all. Refused here rather than at the edge
        // and answered as though it had been taken, for the same reason the
        // line above is: this route does not say which addresses have an
        // account, and a gate that answered differently would be what says it.
        if !email_address::allowed(email) {
            tracing::warn!(
                email,
                "a sign-in was asked for by an address that may not sign up"
            );

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
            call_id: self.calling(request_id, now),
            action: ActionStore::SaveCode {
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
            call_id: self.calling(request_id, now),
            action: ActionStore::LoadCode {
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
                call_id: self.calling(request_id, now),
                action: ActionStore::RecordAttempt {
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
            call_id: self.calling(request_id, now),
            action: ActionStore::CreateSession {
                email: email.to_string(),
                token_sha256: session_token::hash(&token),
                expires_at: Timestamp::from_secs(now.as_secs() + SESSION_GOOD_FOR_SECS),
            },
        });

        // Single use, and spent only once it has bought something.
        actions.push(Action::ToStore {
            call_id: self.calling(request_id, now),
            action: ActionStore::DeleteCode {
                email: email.to_string(),
            },
        });

        self.answer(request_id, ActionRouter::SignedIn { token }, now, actions);
    }

    /// Somebody wants a bank sign-in opened. Nothing to read: their id, and
    /// the `Origin` they asked from, are the whole of what it takes.
    ///
    /// The budget is the account's, which is why it is passed in:
    /// [`StateServer`](crate::StateServer) holds one for every request this
    /// process sends Plaid.
    pub fn link_requested(
        &mut self,
        request_id: RequestId,
        user_id: Uuid,
        origin: Option<&str>,
        limiter: &mut PlaidRateLimiter,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        if !may_send(limiter, PlaidEndpoint::LinkTokenCreate, now) {
            tracing::warn!(%user_id, "a bank sign-in was not opened: too many this minute");

            self.answer(request_id, ActionRouter::TooMany, now, actions);

            return;
        }

        self.waits_on(request_id, Pending::Opening);

        actions.push(Action::ToSourcePlaid {
            call_id: self.calling(request_id, now),
            action: ActionSourcePlaid::OpenLink {
                user_id,
                opener: self
                    .web_origins
                    .as_ref()
                    .map_or(LinkOpener::App, |web| LinkOpener::of(origin, web)),
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
            call_id: self.calling(request_id, now),
            action: ActionStore::LoadPendingLinks { user_id },
        });
    }

    /// What a sheet closing came to, now that the open sessions are known.
    pub fn pending_links_loaded(
        &mut self,
        request_id: RequestId,
        link_tokens: &[String],
        limiter: &mut PlaidRateLimiter,
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

        // Asked here rather than on the way in: a close that has nothing to
        // poll about sends no request, and must not spend the budget for one.
        if !may_send(limiter, PlaidEndpoint::LinkSessionGet, now) {
            tracing::warn!("a link session was not polled: too many this minute");

            self.answer(request_id, ActionRouter::TooMany, now, actions);

            return;
        }

        self.waits_on(request_id, Pending::Polling);

        actions.push(Action::ToSourcePlaid {
            call_id: self.calling(request_id, now),
            action: ActionSourcePlaid::PollLinks {
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
            call_id: self.calling(request_id, now),
            action: ActionStore::CollectLink {
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
            call_id: self.calling(request_id, now),
            action: ActionStore::SaveLink {
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
        limiter: &mut PlaidRateLimiter,
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
            // The narrowest budget there is, because each of these that lands
            // creates an Item nothing gives back. Nothing goes out and the
            // session stays open, because collecting one is what reporting an
            // outcome does and there is no outcome.
            if !may_send(limiter, PlaidEndpoint::PublicTokenExchange, now) {
                tracing::error!(link_token, "a bank was connected past the exchange budget");

                self.answer(request_id, ActionRouter::TooMany, now, actions);

                return;
            }

            self.waits_on(request_id, Pending::Exchanging);

            actions.push(Action::ToSourcePlaid {
                call_id: self.exchanges(request_id, now),
                action: ActionSourcePlaid::ExchangePublicToken {
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
                call_id: self.calling(request_id, now),
                action: ActionStore::CollectLink { link_token },
            });
        }

        self.answer(request_id, ActionRouter::LinkAbandoned, now, actions);
    }

    /// A bank is attached at Plaid, and the write that attaches it here goes
    /// out.
    ///
    /// Nothing is answered on this turn. What a route is owed depends on
    /// whether the row appears, and the only thing that knows is the write:
    /// the exchange behind this spent an Item slot, and saying it worked
    /// before anything was written is saying it about a bank nothing here can
    /// reach.
    pub fn item_linked(
        &mut self,
        call_id: CallId,
        link_token: &str,
        item_id: &str,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        self.exchanged(call_id);

        // Named by the call and not by a route, because there may no longer be
        // one: an exchange outlives the request that asked for it, and the
        // bank it produced is written down either way.
        let write = self.passing(call_id, now);

        self.attaches(
            write,
            Attaching {
                item_id: item_id.to_string(),
                link_token: link_token.to_string(),
                attempt: 1,
            },
            actions,
        );

        // A webhook arrives unprompted, so the session has to stand until
        // `/link/finish` reports it.
        if !self.collects_by_webhook() {
            actions.push(Action::ToStore {
                call_id: self.alongside(write, now),
                action: ActionStore::CollectLink {
                    link_token: link_token.to_string(),
                },
            });
        }
    }

    /// Ask the store to write one bank down, and keep what it would take to
    /// ask again.
    fn attaches(&mut self, call_id: CallId, attaching: Attaching, actions: &mut Vec<Action>) {
        actions.push(Action::ToStore {
            call_id,
            action: ActionStore::SaveItem {
                item_id: attaching.item_id.clone(),
                link_token: attaching.link_token.clone(),
            },
        });

        self.attaching.insert(call_id, attaching);
    }

    /// The bank is written down, which is the first turn in which anybody can
    /// be told a bank was linked.
    pub fn item_attached(&mut self, call_id: CallId, now: Timestamp, actions: &mut Vec<Action>) {
        let Some(attaching) = self.attaching.remove(&call_id) else {
            tracing::error!(call_id, "the store wrote a bank nobody asked it to");

            return;
        };

        // Plaid's POST is owed nothing beyond "taken": the outcome goes to
        // whichever `/link/finish` asks for it, now or in a moment.
        if self.collects_by_webhook() {
            self.settled(&attaching.link_token, LinkOutcome::Linked, now, actions);
        } else if let Some(request_id) = self.waiter(call_id) {
            self.linked_one(request_id, now, actions);
        }

        self.landed(call_id, now, actions);
    }

    /// The store had nothing to write down: no access token held for the bank,
    /// or no Link session to attach it to. Neither of those becomes true
    /// later, so this is the end of it.
    pub fn item_unattached(&mut self, call_id: CallId, now: Timestamp, actions: &mut Vec<Action>) {
        let Some(attaching) = self.attaching.remove(&call_id) else {
            tracing::error!(call_id, "the store answered a write nobody asked for");

            return;
        };

        self.attach_given_up(call_id, &attaching, now, actions);
    }

    /// The store refused the write, and whether it was one of these is the
    /// answer. An ordinary write lands or does not; this one is asked for
    /// again, because the slot the exchange spent cannot be spent twice.
    pub fn attach_refused(
        &mut self,
        call_id: CallId,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) -> bool {
        let Some(attaching) = self.attaching.remove(&call_id) else {
            return false;
        };

        if attaching.attempt >= ATTACH_ATTEMPTS {
            self.attach_given_up(call_id, &attaching, now, actions);

            return true;
        }

        tracing::warn!(
            item_id = %attaching.item_id,
            attempt = attaching.attempt,
            "a linked bank was not written down; asking again",
        );

        // The wait moves onto the new call, so the route is answered at the
        // end of the chain rather than by a failure partway along it.
        let next = self.passing(call_id, now);

        self.attaches(
            next,
            Attaching {
                attempt: attaching.attempt + 1,
                ..attaching
            },
            actions,
        );

        true
    }

    /// A bank that is live at Plaid and written down nowhere.
    ///
    /// The route hears that this server could not answer, which is neither
    /// that a bank was attached nor that none was. `LinkAbandoned` would be
    /// the second of those, and it is a sentence about somebody's bank.
    ///
    /// The Link session is left as it stands. On a server Plaid calls, one
    /// that was never settled is one the webhook can report again.
    fn attach_given_up(
        &mut self,
        call_id: CallId,
        attaching: &Attaching,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        tracing::error!(
            item_id = %attaching.item_id,
            "a linked bank could not be written down; its Item slot is spent",
        );

        if let Some(request_id) = self.waiter(call_id) {
            self.answer(request_id, ActionRouter::Unavailable, now, actions);
        }

        self.landed(call_id, now, actions);
    }

    /// An exchange came to nothing, which is also the one ending in which no
    /// Item slot was spent: Plaid refused.
    pub fn link_abandoned(
        &mut self,
        call_id: CallId,
        link_token: &str,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        self.exchanged(call_id);

        // An exchange that failed, on a session Plaid told us about: nothing
        // came of it, and somebody is waiting to hear that.
        if self.collects_by_webhook() && !link_token.is_empty() {
            if let Some(request_id) = self.waiter(call_id) {
                self.answer(request_id, ActionRouter::Accepted, now, actions);
            }

            self.landed(call_id, now, actions);

            self.settled(link_token, LinkOutcome::Nothing, now, actions);

            return;
        }

        let collect = self.passing(call_id, now);

        actions.push(Action::ToStore {
            call_id: collect,
            action: ActionStore::CollectLink {
                link_token: link_token.to_string(),
            },
        });

        if let Some(request_id) = self.waiter(collect) {
            self.answer(request_id, ActionRouter::LinkAbandoned, now, actions);
        }
    }

    /// Plaid says a Link session is over, and hands us what it produced.
    pub fn session_finished(
        &mut self,
        request_id: RequestId,
        link_token: &str,
        public_tokens: &[String],
        limiter: &mut PlaidRateLimiter,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        // Nothing left to trade: they closed the sheet, or there is no slot
        // for what they produced. An outcome Plaid *said*, not one inferred
        // from an empty poll.
        if public_tokens.is_empty() {
            self.answer(request_id, ActionRouter::Accepted, now, actions);

            self.settled(link_token, LinkOutcome::Nothing, now, actions);

            return;
        }

        // Every token, because Plaid says this once: one nobody exchanges is a
        // bank somebody connected and does not get. One request each, so the
        // budget is asked once each — and this is the endpoint where a burst
        // is the thing worth stopping.
        for public_token in public_tokens {
            if !may_send(limiter, PlaidEndpoint::PublicTokenExchange, now) {
                tracing::error!(link_token, "a bank was connected past the exchange budget");

                break;
            }

            actions.push(Action::ToSourcePlaid {
                call_id: self.exchanges(request_id, now),
                action: ActionSourcePlaid::ExchangePublicToken {
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
            call_id: self.call(),
            action: ActionStore::SweepSessions { now },
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

    /// A fresh [`CallId`] for the one call that cannot be taken back, noted
    /// apart from the wait so that letting the route go does not lose it.
    fn exchanges(&mut self, request_id: RequestId, now: Timestamp) -> CallId {
        let call_id = self.calling(request_id, now);

        self.exchanging.insert(call_id, now);

        call_id
    }

    /// The exchange answered, whichever way. Silent on a call that was not
    /// one: a link Plaid would not open reports the same event.
    fn exchanged(&mut self, call_id: CallId) {
        self.exchanging.remove(&call_id);
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

    /// The route parked on a call, without letting go of the wait. For a
    /// handler that has more to do before the call is done with.
    fn waiter(&self, call_id: CallId) -> Option<RequestId> {
        self.waiting.get(&call_id).map(|waiter| waiter.request_id)
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
                    call_id: self.calling(request_id, now),
                    action: ActionStore::LoadSnapshot { user_id },
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

            // Another of this route's calls may still answer and release it.
            if self.outstanding(request_id) {
                continue;
            }

            // The exchange is not given up on, only stopped being waited for.
            // A response that is slow or lost says nothing about whether Plaid
            // created the Item, and the answer carries the only access token
            // there will ever be — so the route is let go of and the call
            // stays out. `LinkPending` on both roads: the phone reads it as
            // "not known yet", and Plaid reads it as a 503 and sends again.
            if self.exchanging.contains_key(&call_id) {
                tracing::warn!(
                    call_id,
                    request_id,
                    "an exchange is taking a while; the route is let go of and the call is not",
                );

                self.requests.remove(&request_id);

                self.release(
                    request_id,
                    Owed::Answer(ActionRouter::LinkPending),
                    now,
                    actions,
                );

                continue;
            }

            tracing::warn!(
                call_id,
                request_id,
                "a call took too long and was given up on"
            );

            // A write that never answered is a bank left unwritten, and what
            // it would take to ask again goes with the call it was out on.
            if let Some(attaching) = self.attaching.remove(&call_id) {
                tracing::error!(
                    item_id = %attaching.item_id,
                    "a linked bank's write never answered; its Item slot is spent",
                );
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

        self.exchanges_given_up(now);
    }

    /// Stop holding the process open for an exchange that is never going to
    /// answer. Nothing is recovered by this and nothing can be: the line is
    /// the only record that a slot may have gone.
    fn exchanges_given_up(&mut self, now: Timestamp) {
        let quiet: Vec<CallId> = self
            .exchanging
            .iter()
            .filter(|(_, since)| now.since(**since).as_secs() >= EXCHANGE_GOOD_FOR_SECS)
            .map(|(call_id, _)| *call_id)
            .collect();

        for call_id in quiet {
            self.exchanging.remove(&call_id);

            tracing::error!(
                call_id,
                "an exchange never answered; if it made an Item, that slot is spent",
            );
        }
    }

    /// Whether nothing is out and no route is in flight. The machine cannot
    /// come to a stop before this is true.
    ///
    /// The two tables beyond the obvious ones are the halves of attaching a
    /// bank, and each is a call the machine started that no route may still be
    /// waiting on. Stopping with one of them full is stopping on a spent Item
    /// slot.
    pub fn quiet(&self) -> bool {
        self.waiting.is_empty()
            && self.requests.is_empty()
            && self.exchanging.is_empty()
            && self.attaching.is_empty()
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

    /// How many exchanges are out, each one an Item slot that may be spent.
    pub fn exchanging(&self) -> usize {
        self.exchanging.len()
    }

    /// How many banks are waiting on the write that attaches them.
    pub fn attaching(&self) -> usize {
        self.attaching.len()
    }
}

/// Whether Plaid's budget for an endpoint that names no bank has room, and
/// spend it if it has.
///
/// Every request this machine sends Plaid goes through it. A free function
/// rather than a method: the budget belongs to the process, not to signing up.
fn may_send(limiter: &mut PlaidRateLimiter, endpoint: PlaidEndpoint, now: Timestamp) -> bool {
    limiter.can_send(now, &PlaidRateCtx::account(endpoint))
}
