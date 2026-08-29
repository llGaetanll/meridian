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
//! whether the process is shutting down. Everything else arrives on the event
//! that needs it, and is served from the store.
//!
//! So "should I sync this bank?" is a pure function of facts handed in and a
//! clock handed in, which is what makes a stale-cache bug reproducible from a
//! log rather than from a stopwatch.

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
use crate::Event;
use crate::EventProcess;
use crate::EventRouter;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;
use crate::ItemFacts;
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
/// How long a session lasts. Long, because the alternative is asking somebody
/// to check their email to look at their own balance; short enough that a
/// phone lost in a drawer stops working eventually.
pub const SESSION_GOOD_FOR_SECS: u64 = 90 * 24 * 60 * 60;

#[derive(Debug, Default)]
pub struct StateServer {
    /// Where sign-in codes and session tokens come from.
    ///
    /// Seeded from the edge by [`EventProcess::Seeded`](crate::EventProcess),
    /// never from a clock or a counter, and `None` until one arrives. ChaCha20
    /// rather than a fast PRNG: these are credentials, and a generator whose
    /// state can be recovered from a couple of outputs would hand an attacker
    /// every code after the ones they saw.
    ///
    /// In the machine rather than the runtime so that a code is a *decision*
    /// like everything else — reproducible from the event log, and reachable
    /// from an `eval!` test.
    rng: Option<ChaCha20Rng>,

    /// What Plaid will let us send. The one thing here that genuinely cannot
    /// be read back from the store — it is a fact about this process's last
    /// minute, and it is why "may I call?" is arithmetic in the machine rather
    /// than a guess in the runtime.
    limiter: PlaidRateLimiter,

    /// Whether a shutdown has been asked for. Once set, no new work starts.
    shutting_down: bool,

    /// When the current batch is being handled.
    now: Timestamp,
}

impl StateServer {
    pub fn new() -> Self {
        Self::default()
    }

    /// The only entry point.
    ///
    /// `now` arrives once per batch rather than on every event: a batch happens
    /// at one instant, and an event carrying its own clock reading would be a
    /// reading the runtime could get wrong.
    pub fn handle_events(&mut self, now: Timestamp, events: &[Event], actions: &mut Vec<Action>) {
        debug_assert!(
            now >= self.now,
            "time went backwards: {now} after {}",
            self.now,
        );

        self.now = now;

        for event in events {
            self.handle_event(event, actions);
        }
    }

    fn handle_event(&mut self, event: &Event, actions: &mut Vec<Action>) {
        match event {
            Event::FromRouter { event } => self.handle_event_router(event, actions),
            Event::FromSource { event } => self.handle_event_source(event, actions),
            Event::FromStore { event } => self.handle_event_store(event, actions),
            Event::FromProcess { event } => self.handle_event_process(event, actions),
            Event::FromTerminal { event } => self.handle_event_terminal(event, actions),
        }
    }

    fn handle_event_router(&mut self, event: &EventRouter, actions: &mut Vec<Action>) {
        match event {
            // Free to ask for, and creates nothing at Plaid's end — a session
            // nobody finishes costs nothing and expires on its own. So there
            // is nothing to rate limit here that is not already covered by
            // issue #23.
            EventRouter::LinkRequested { user_id } => {
                actions.push(Action::ToSource {
                    action: ActionSource::OpenLink { user_id: *user_id },
                });
            }

            EventRouter::LinkClosed { pending } => {
                // Nothing open. Somebody closed a sheet we have no record of,
                // or collected this one already — either way there is nothing
                // to go and ask about.
                if pending.is_empty() {
                    actions.push(Action::ToRouter {
                        action: ActionRouter::NotLinked,
                    });

                    return;
                }

                actions.push(Action::ToSource {
                    action: ActionSource::PollLinks {
                        link_tokens: pending.clone(),
                    },
                });
            }

            EventRouter::CodeRequested { email } => {
                // No entropy, no credentials. Refused rather than filled in
                // with anything derivable — a predictable sign-in code is a
                // way into somebody's bank, and there is no fallback that is
                // better than saying no.
                let Some(rng) = self.rng.as_mut() else {
                    tracing::error!("asked for a sign-in code before the machine was seeded");

                    actions.push(Action::ToRouter {
                        action: ActionRouter::Unavailable,
                    });

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

                actions.push(Action::ToRouter {
                    action: ActionRouter::Accepted,
                });
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
                    actions.push(Action::ToRouter {
                        action: ActionRouter::Rejected,
                    });

                    return;
                };

                let verdict = login_code::verify(stored, code, self.now);

                if verdict != login_code::Verdict::Accepted {
                    // Counted whatever the reason, including expired: an
                    // attacker who could reset the cap by waiting would have
                    // an unlimited number of guesses at the next code.
                    actions.push(Action::ToStore {
                        action: ActionStore::RecordAttempt {
                            email: email.clone(),
                        },
                    });

                    tracing::warn!(email, ?verdict, "a sign-in code was rejected");

                    actions.push(Action::ToRouter {
                        action: ActionRouter::Rejected,
                    });

                    return;
                }

                let Some(rng) = self.rng.as_mut() else {
                    tracing::error!("a code was accepted before the machine was seeded");

                    actions.push(Action::ToRouter {
                        action: ActionRouter::Rejected,
                    });

                    return;
                };

                let token = login_code::generate_token(rng);

                actions.push(Action::ToStore {
                    action: ActionStore::CreateSession {
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
                        email: email.clone(),
                    },
                });

                actions.push(Action::ToRouter {
                    action: ActionRouter::SignedIn { token },
                });
            }

            EventRouter::SnapshotRequested { force, items } => {
                for item in items {
                    if self.should_fetch(item, *force) {
                        actions.push(Action::ToSource {
                            action: ActionSource::FetchPlaid {
                                item_id: item.item_id.clone(),
                                cursor: item.cursor.clone(),
                            },
                        });
                    }
                }

                // Always, and last. Whatever was fetched has been written by
                // the time this is carried out, and a user with no banks gets
                // the empty ledger rather than an error — that is what "no
                // bank linked yet" looks like, and it is the state first run
                // exists to leave.
                actions.push(Action::ToRouter {
                    action: ActionRouter::Serve,
                });
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
                item_id,
                added,
                modified,
                removed,
                connection,
                streams,
                cursor,
            } => {
                actions.push(Action::ToStore {
                    action: ActionStore::SaveFetched {
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
                user_id,
                link_token,
                hosted_link_url,
            } => {
                // Written down before the URL is handed over. The other order
                // leaves a window where somebody could finish a session we
                // have no record of asking for.
                actions.push(Action::ToStore {
                    action: ActionStore::SaveLink {
                        link_token: link_token.clone(),
                        user_id: *user_id,
                    },
                });

                actions.push(Action::ToRouter {
                    action: ActionRouter::LinkOpened {
                        url: hosted_link_url.clone(),
                    },
                });
            }

            EventSource::LinksPolled { polls } => {
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
                    actions.push(Action::ToSource {
                        action: ActionSource::ExchangePublicToken {
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
                            link_token: poll.link_token.clone(),
                        },
                    });
                }

                actions.push(Action::ToRouter {
                    action: ActionRouter::NotLinked,
                });
            }

            EventSource::ItemLinked {
                link_token,
                item_id,
            } => {
                actions.push(Action::ToStore {
                    action: ActionStore::SaveItem {
                        item_id: item_id.clone(),
                        link_token: link_token.clone(),
                    },
                });

                actions.push(Action::ToStore {
                    action: ActionStore::CollectLink {
                        link_token: link_token.clone(),
                    },
                });

                actions.push(Action::ToRouter {
                    action: ActionRouter::Linked,
                });
            }

            EventSource::LinkAbandoned { link_token } => {
                actions.push(Action::ToStore {
                    action: ActionStore::CollectLink {
                        link_token: link_token.clone(),
                    },
                });

                actions.push(Action::ToRouter {
                    action: ActionRouter::NotLinked,
                });
            }

            EventSource::PlaidFailed { item_id, code } => {
                if code == "ITEM_LOGIN_REQUIRED" {
                    actions.push(Action::ToProcess {
                        action: ActionProcess::ReportLoginRequired,
                    });

                    // Written down, or the server comes back up and asks Plaid
                    // the same question it was just refused.
                    actions.push(Action::ToStore {
                        action: ActionStore::SaveStatus {
                            item_id: item_id.clone(),
                            error_code: Some(code.clone()),
                        },
                    });
                }
            }
        }
    }

    fn handle_event_process(&mut self, event: &EventProcess, _actions: &mut Vec<Action>) {
        match event {
            // Nothing to do. The store is read per request, for the user who
            // asked, so there is nothing to pull into memory at boot — which
            // is what makes a second server process possible.
            EventProcess::Started => {}

            EventProcess::Seeded { seed } => {
                self.rng = Some(ChaCha20Rng::from_seed(*seed));
            }
        }
    }

    fn handle_event_store(&mut self, event: &EventStore, _actions: &mut Vec<Action>) {
        match event {
            // Both carry on. They are events rather than a line in the runtime
            // because the log is meant to be a recording of the session, and
            // "the store was unreachable" is part of the story.
            EventStore::LoadFailed { message } => {
                tracing::error!(message, "could not read the store");
            }

            EventStore::SaveFailed { message } => {
                tracing::error!(message, "could not write the store");
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
                if self.shutting_down {
                    return;
                }

                self.shutting_down = true;

                actions.push(Action::ToProcess {
                    action: ActionProcess::Shutdown,
                });
            }
        }
    }

    /// Whether to go back to Plaid about this bank.
    ///
    /// Pure, and reads the clock it was handed rather than one of its own —
    /// the whole reason this lives in a state machine.
    fn should_fetch(&mut self, item: &ItemFacts, force: bool) -> bool {
        // Nothing new starts once we are on the way out.
        if self.shutting_down {
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

    /// The clock the last batch was handled at.
    ///
    /// Public because `eval!` reads it: a test sets the time it wants before
    /// driving the machine, rather than the macro carrying a clock of its own.
    pub fn now(&self) -> Timestamp {
        self.now
    }

    /// Move the clock, for tests that are *about* time — staleness, backoff.
    ///
    /// Nothing in the running program calls this: time arrives solely through
    /// `handle_events`, from the runtime that read it.
    pub fn set_now(&mut self, now: Timestamp) {
        self.now = now;
    }

    pub fn shutting_down(&self) -> bool {
        self.shutting_down
    }
}
