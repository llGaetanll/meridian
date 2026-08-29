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
use src_plaid::PlaidEndpoint;
use src_plaid::PlaidRateCtx;
use src_plaid::PlaidRateLimiter;

use crate::Action;
use crate::ActionClient;
use crate::ActionProcess;
use crate::ActionSource;
use crate::ActionStore;
use crate::Event;
use crate::EventClient;
use crate::EventProcess;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;
use crate::ItemFacts;

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
#[derive(Debug, Default)]
pub struct StateServer {
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
            Event::FromClient { event } => self.handle_event_client(event, actions),
            Event::FromSource { event } => self.handle_event_source(event, actions),
            Event::FromStore { event } => self.handle_event_store(event, actions),
            Event::FromProcess { event } => self.handle_event_process(event, actions),
            Event::FromTerminal { event } => self.handle_event_terminal(event, actions),
        }
    }

    fn handle_event_client(&mut self, event: &EventClient, actions: &mut Vec<Action>) {
        match event {
            EventClient::SnapshotRequested { force, items } => {
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
                actions.push(Action::ToClient {
                    action: ActionClient::Serve,
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
