//! Signing up: the code, the session it buys, and the bank sign-in that
//! follows.
//!
//! Everything that gets somebody from an empty screen to a connected bank, and
//! nothing that happens afterwards. It holds what that takes — the generator
//! codes and tokens are drawn from, and what each Link session came to — and
//! decides what its own events mean. [`StateServer`](crate::StateServer)
//! routes them here; `handle_events` is still the only way anything changes.
//!
//! The hard part is the bank sign-in, and it is hard because two things race
//! with no ordering between them: the redirect that closes Plaid's sheet, and
//! Plaid's POST saying what the session came to. Either can arrive first, so
//! whichever it is waits for the other.

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
use crate::LinkOutcome;
use crate::LinkPoll;
use crate::Owed;
use crate::Pending;
use crate::PlaidCollect;
use crate::RequestId;
use crate::Settled;
use crate::StoredCode;
use crate::Turn;
use crate::Watching;
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

    /// What each Link session came to, for a route that has not asked yet,
    /// keyed by the `link_token` that `SESSION_FINISHED` carries. Always empty
    /// under `PLAID_COLLECT=poll`, where the close causes the collection.
    outcomes: HashMap<String, Settled>,

    /// The other order: a route that asked before Plaid said anything.
    /// [`outcomes`](Self::outcomes) is the half that arrived first.
    watching: HashMap<String, Watching>,
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
    pub fn code_requested(&mut self, request_id: RequestId, email: &str, turn: &mut Turn) {
        // Answered as though it had been taken: the route says the same thing
        // whether or not an address has an account, and the shape of the
        // string must not say otherwise.
        if !email_address::plausible(email) {
            tracing::warn!("a sign-in was asked for with something that is not an address",);

            turn.answer(request_id, ActionRouter::Accepted);

            return;
        }

        let Some(rng) = self.rng.as_mut() else {
            tracing::error!("asked for a sign-in code by a machine that mints none");

            turn.answer(request_id, ActionRouter::Unavailable);

            return;
        };

        let code = login_code::generate(rng);

        let expires_at =
            Timestamp::from_secs(turn.now().as_secs() + login_code::GOOD_FOR.as_secs());

        // Two actions because they go to different places, and only one of
        // them may ever hold the plaintext.
        let call_id = turn.calling(request_id);

        turn.push(Action::ToStore {
            action: ActionStore::SaveCode {
                call_id,
                email: email.to_string(),
                code_sha256: login_code::hash(&code),
                expires_at,
            },
        });

        turn.push(Action::ToEmail {
            action: ActionEmail::SendCode {
                email: email.to_string(),
                code,
            },
        });

        turn.answer(request_id, ActionRouter::Accepted);
    }

    /// Somebody typed a code. What they typed is kept, because the store has
    /// to be asked what it holds against the address first.
    pub fn code_submitted(
        &mut self,
        request_id: RequestId,
        email: &str,
        code: &str,
        turn: &mut Turn,
    ) {
        let call_id = turn.calling(request_id);

        turn.waits_on(
            call_id,
            Pending::Submitted {
                email: email.to_string(),
                code: code.to_string(),
            },
        );

        turn.push(Action::ToStore {
            action: ActionStore::LoadCode {
                call_id,
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
        turn: &mut Turn,
    ) {
        // No code standing against this address, and a rejection like any
        // other: telling them apart says which addresses have one in flight.
        let Some(stored) = stored else {
            turn.answer(request_id, ActionRouter::Rejected);

            return;
        };

        let verdict = login_code::verify(stored, code, turn.now());

        if verdict != login_code::Verdict::Accepted {
            // Counted whatever the reason, expiry included: resetting the cap
            // by waiting would give unlimited guesses at the next code.
            let call_id = turn.calling(request_id);

            turn.push(Action::ToStore {
                action: ActionStore::RecordAttempt {
                    call_id,
                    email: email.to_string(),
                },
            });

            tracing::warn!(email, ?verdict, "a sign-in code was rejected");

            turn.answer(request_id, ActionRouter::Rejected);

            return;
        }

        let Some(rng) = self.rng.as_mut() else {
            tracing::error!("a code was accepted by a machine that mints none");

            turn.answer(request_id, ActionRouter::Rejected);

            return;
        };

        let token = login_code::generate_token(rng);

        let expires_at = Timestamp::from_secs(turn.now().as_secs() + SESSION_GOOD_FOR_SECS);
        let call_id = turn.calling(request_id);

        turn.push(Action::ToStore {
            action: ActionStore::CreateSession {
                call_id,
                email: email.to_string(),
                token_sha256: session_token::hash(&token),
                expires_at,
            },
        });

        // Single use, and spent only once it has bought something.
        let call_id = turn.calling(request_id);

        turn.push(Action::ToStore {
            action: ActionStore::DeleteCode {
                call_id,
                email: email.to_string(),
            },
        });

        turn.answer(request_id, ActionRouter::SignedIn { token });
    }

    /// Somebody wants a bank sign-in opened. Nothing to read: their id is the
    /// whole of what it takes.
    pub fn link_requested(&mut self, request_id: RequestId, user_id: Uuid, turn: &mut Turn) {
        let call_id = turn.calling(request_id);

        turn.push(Action::ToSource {
            action: ActionSource::OpenLink { call_id, user_id },
        });
    }

    /// A sheet closed. Which sessions this caller has open decides what that
    /// means, so the store is asked first.
    pub fn link_closing(&mut self, request_id: RequestId, user_id: Uuid, turn: &mut Turn) {
        let call_id = turn.calling(request_id);

        turn.waits_on(
            call_id,
            Pending::Loading {
                user_id,
                event: Authenticated::LinkClosed,
            },
        );

        turn.push(Action::ToStore {
            action: ActionStore::LoadPendingLinks { call_id, user_id },
        });
    }

    /// What a sheet closing came to, now that the open sessions are known.
    pub fn pending_links_loaded(
        &mut self,
        request_id: RequestId,
        link_tokens: &[String],
        turn: &mut Turn,
    ) {
        // Collection already happens unprompted, so this route asks rather
        // than triggers: Plaid would charge a call for a token already gone.
        if self.collects_by_webhook() {
            self.closed_by_webhook(request_id, link_tokens, turn);

            return;
        }

        // Nothing open, so nothing to go and ask about.
        if link_tokens.is_empty() {
            turn.answer(request_id, ActionRouter::LinkAbandoned);

            return;
        }

        let call_id = turn.calling(request_id);

        turn.push(Action::ToSource {
            action: ActionSource::PollLinks {
                call_id,
                link_tokens: link_tokens.to_vec(),
            },
        });
    }

    /// A sheet closed, on a server Plaid calls.
    ///
    /// The redirect that closed it says nothing — Plaid's documentation is
    /// explicit that reaching the completion URI is not a connection, and that
    /// the answer is `SESSION_FINISHED` — so this route only asks.
    fn closed_by_webhook(&mut self, request_id: RequestId, pending: &[String], turn: &mut Turn) {
        // Looked for before anybody is parked, because the POST usually wins
        // the race and waiting first would make it sit until it timed out.
        if let Some((link_token, outcome)) = self.collected(pending) {
            self.report(request_id, &link_token, outcome, turn);

            return;
        }

        // Nothing open and nothing recorded: parking would promise nothing.
        if pending.is_empty() {
            turn.answer(request_id, ActionRouter::LinkAbandoned);

            return;
        }

        // Parked on every session the caller has open. The first to report
        // answers, and the timeout is what stops this being forever.
        for link_token in pending {
            let displaced = self.watching.insert(
                link_token.clone(),
                Watching {
                    request_id,
                    since: turn.now(),
                },
            );

            // Two calls about the same session: the older is let go rather
            // than parked on a key it does not own, which nothing answers.
            if let Some(displaced) = displaced
                && displaced.request_id != request_id
            {
                turn.answer(displaced.request_id, ActionRouter::LinkPending);
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
        turn: &mut Turn,
    ) {
        // An entry left behind sends the next settlement to a dead request.
        self.watching
            .retain(|_, watching| watching.request_id != request_id);

        let call_id = turn.calling(request_id);

        turn.push(Action::ToStore {
            action: ActionStore::CollectLink {
                call_id,
                link_token: link_token.to_string(),
            },
        });

        turn.answer(request_id, outcome.answer());
    }

    /// One Link session has come to something, on a server Plaid calls.
    /// Either a route is parked on it or one will ask, and this is the one
    /// place that does not care which.
    fn settled(&mut self, link_token: &str, outcome: LinkOutcome, turn: &mut Turn) {
        // No token means `/link/start` failed before there was one.
        if link_token.is_empty() {
            return;
        }

        let Some(watching) = self.watching.remove(link_token) else {
            self.outcomes.insert(
                link_token.to_string(),
                Settled {
                    outcome,
                    at: turn.now(),
                },
            );

            return;
        };

        self.report(watching.request_id, link_token, outcome, turn);
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
        turn: &mut Turn,
    ) {
        // Written down first, or somebody could finish a session we have no
        // record of asking for.
        let call_id = turn.calling(request_id);

        turn.push(Action::ToStore {
            action: ActionStore::SaveLink {
                call_id,
                link_token: link_token.to_string(),
                user_id,
            },
        });

        turn.answer(
            request_id,
            ActionRouter::LinkOpened {
                url: hosted_link_url.to_string(),
            },
        );
    }

    /// What Plaid says about the sessions this caller had open, on a server
    /// Plaid does not call.
    pub fn links_polled(&mut self, request_id: RequestId, polls: &[LinkPoll], turn: &mut Turn) {
        // A token is the answer whether or not the session says it has
        // finished. The first wins and the rest wait for the next close,
        // because the route can be told about one.
        let exchangeable = polls
            .iter()
            .find_map(|poll| Some((&poll.link_token, poll.public_tokens.first()?)));

        if let Some((link_token, public_token)) = exchangeable {
            let call_id = turn.calling(request_id);

            turn.push(Action::ToSource {
                action: ActionSource::ExchangePublicToken {
                    call_id,
                    link_token: link_token.clone(),
                    public_token: public_token.clone(),
                },
            });

            return;
        }

        // They closed the sheet, and it is marked done so it is not polled
        // forever. One still running is somebody typing.
        for poll in polls.iter().filter(|poll| poll.finished) {
            let call_id = turn.calling(request_id);

            turn.push(Action::ToStore {
                action: ActionStore::CollectLink {
                    call_id,
                    link_token: poll.link_token.clone(),
                },
            });
        }

        turn.answer(request_id, ActionRouter::LinkAbandoned);
    }

    /// A bank is attached. The first sync follows this write rather than
    /// racing it: whose bank it is comes from the Link session, and the write
    /// is what resolves that.
    pub fn item_linked(
        &mut self,
        request_id: RequestId,
        link_token: &str,
        item_id: &str,
        turn: &mut Turn,
    ) {
        let call_id = turn.calling(request_id);

        turn.push(Action::ToStore {
            action: ActionStore::SaveItem {
                call_id,
                item_id: item_id.to_string(),
                link_token: link_token.to_string(),
            },
        });

        // A webhook arrives unprompted, so the session has to stand until
        // `/link/finish` reports it.
        if !self.collects_by_webhook() {
            let call_id = turn.calling(request_id);

            turn.push(Action::ToStore {
                action: ActionStore::CollectLink {
                    call_id,
                    link_token: link_token.to_string(),
                },
            });
        }

        // Plaid's POST caused this and is owed nothing beyond "taken": the
        // outcome goes to `/link/finish`.
        if self.collects_by_webhook() {
            turn.answer(request_id, ActionRouter::Accepted);

            self.settled(link_token, LinkOutcome::Linked, turn);

            return;
        }

        self.linked_one(request_id, turn);
    }

    /// An exchange came to nothing.
    pub fn link_abandoned(&mut self, request_id: RequestId, link_token: &str, turn: &mut Turn) {
        // An exchange that failed, on a session Plaid told us about: nothing
        // came of it, and somebody is waiting to hear that.
        if self.collects_by_webhook() && !link_token.is_empty() {
            turn.answer(request_id, ActionRouter::Accepted);

            self.settled(link_token, LinkOutcome::Nothing, turn);

            return;
        }

        let call_id = turn.calling(request_id);

        turn.push(Action::ToStore {
            action: ActionStore::CollectLink {
                call_id,
                link_token: link_token.to_string(),
            },
        });

        turn.answer(request_id, ActionRouter::LinkAbandoned);
    }

    /// Plaid says a Link session is over, and hands us what it produced.
    pub fn session_finished(
        &mut self,
        request_id: RequestId,
        link_token: &str,
        public_tokens: &[String],
        turn: &mut Turn,
    ) {
        // Finished with nothing: they closed the sheet. An outcome Plaid
        // *said*, not one inferred from an empty poll.
        if public_tokens.is_empty() {
            turn.answer(request_id, ActionRouter::Accepted);

            self.settled(link_token, LinkOutcome::Nothing, turn);

            return;
        }

        // Every token, because Plaid says this once: one nobody exchanges is a
        // bank somebody connected and does not get.
        for public_token in public_tokens {
            let call_id = turn.calling(request_id);

            turn.push(Action::ToSource {
                action: ActionSource::ExchangePublicToken {
                    call_id,
                    link_token: link_token.to_string(),
                    public_token: public_token.clone(),
                },
            });
        }

        // Held until the exchanges have landed and been written, so a failure
        // mid-exchange is a webhook Plaid sends again.
        turn.answer(request_id, ActionRouter::Accepted);
    }

    /// One more bank attached on this request. Counted rather than set,
    /// because one `/link/finish` can attach two banks, turns apart; the count
    /// rides on the held answer rather than a tally that outlives it.
    fn linked_one(&mut self, request_id: RequestId, turn: &mut Turn) {
        let count = match turn.owed(request_id) {
            Some(Owed::Answer(ActionRouter::Linked { count })) => *count,
            _ => 0,
        };

        turn.answer(request_id, ActionRouter::Linked { count: count + 1 });
    }

    /// Let go of the routes waiting on a Link session Plaid has not reported,
    /// and drop the outcomes nobody ever came back for.
    ///
    /// `Pending` rather than "nothing came of it", because neither is known
    /// yet: the screen that follows it polls `/sync/status`, so a bank landing
    /// a moment later still shows up.
    pub fn timed_out(&mut self, turn: &mut Turn) {
        let now = turn.now();

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

            turn.answer(request_id, ActionRouter::LinkPending);
        }
    }

    /// Ask the store to drop the sessions that have expired, about hourly.
    /// Decided here rather than on a runtime timer, so a log replays it.
    pub fn sweep_sessions(&mut self, turn: &mut Turn) {
        let now = turn.now();

        // Nothing is owed a sweep at boot: expired rows are already inert.
        let Some(swept) = self.swept else {
            self.swept = Some(now);

            return;
        };

        if now.since(swept).as_secs() < SWEEP_SESSIONS_EVERY_SECS {
            return;
        }

        self.swept = Some(now);

        let call_id = turn.call();

        turn.push(Action::ToStore {
            action: ActionStore::SweepSessions { call_id, now },
        });
    }

    /// Whether any route is parked on a session. The machine cannot come to a
    /// stop while one is: it is owed an answer nothing else will send.
    pub fn parked(&self) -> bool {
        !self.watching.is_empty()
    }

    /// How many outcomes are being kept for a route that has not asked. See
    /// [`CensusServer`](crate::CensusServer).
    pub fn outcomes(&self) -> usize {
        self.outcomes.len()
    }

    /// How many routes are parked on a session.
    pub fn watching(&self) -> usize {
        self.watching.len()
    }
}
