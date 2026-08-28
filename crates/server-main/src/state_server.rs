//! The server's own decisions, kept away from its IO.
//!
//! Same shape as the app's machine and for the same reason
//! (`docs/ARCHITECTURE.md`): events in, actions out, nothing in between
//! touching the outside world. The server has fewer decisions than the app,
//! but the ones it has are exactly the kind worth being able to replay — when
//! to go back to Plaid, what to do with an `ITEM_LOGIN_REQUIRED`, whether a
//! cursor is still good, and whether a call would blow the rate limit.
//!
//! What this buys concretely: "should I refetch?" is a pure function of state
//! and a clock reading handed in, so a stale-cache bug is reproducible from a
//! log rather than from a stopwatch.

use api_common::Timestamp;
use api_common::ratelimit::RateLimiter;
use api_ledger::Connection;
use api_ledger::ConnectionStatus;
use api_ledger::Entry;
use api_ledger::Ledger;
use api_ledger::Stream;
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

/// How long a snapshot is served before Plaid is asked again.
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
    /// Everything synced so far. Held across fetches, because each fetch only
    /// says what changed.
    ledger: Ledger,

    connections: Vec<Connection>,

    streams: Vec<Stream>,

    /// Where the last sync got to. The only record of how far we read, so it is
    /// kept even when a later fetch fails.
    cursor: Option<String>,

    /// When the held data was last refreshed. `None` means never.
    fetched_at: Option<Timestamp>,

    /// Set once Plaid says the Item needs a new login. Sticky: retrying is
    /// pointless until the user acts, so the server stops asking.
    login_required: bool,

    /// Whether a shutdown has been asked for. Once set, no new work starts.
    shutting_down: bool,

    /// What Plaid will let us send. Consulted rather than guessed, and part of
    /// the machine because "may I call?" is arithmetic, not IO.
    limiter: PlaidRateLimiter,

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
            EventClient::SnapshotRequested { force } => {
                // Which Item to sync, if there is one at all. No connection
                // means no bank has been linked yet — a real state with one
                // meaning, and the state first run exists to leave. There is
                // nothing to ask Plaid about, so the empty ledger is served
                // and the app draws its empty screen.
                let item_id = self.connections.first().map(|c| c.id.clone());

                match item_id.filter(|id| self.should_fetch(id, *force)) {
                    Some(item_id) => actions.push(Action::ToSource {
                        action: ActionSource::FetchPlaid {
                            item_id,
                            cursor: self.cursor.clone(),
                        },
                    }),

                    None => actions.push(Action::ToClient {
                        action: ActionClient::Serve,
                    }),
                }
            }
        }
    }

    fn handle_event_source(&mut self, event: &EventSource, actions: &mut Vec<Action>) {
        match event {
            EventSource::PlaidLoaded {
                added,
                modified,
                removed,
                connections,
                streams,
                cursor,
            } => {
                // Applied, not assigned. This is the difference between a
                // ledger that accumulates and one that empties itself on the
                // second sync — see `docs/PLAID.md`.
                self.ledger.apply(added.clone(), modified.clone(), removed);

                self.connections = connections.clone();
                self.streams = streams.clone();
                self.cursor = Some(cursor.clone());
                self.fetched_at = Some(self.now);

                // A successful fetch is the only thing that clears this: it
                // proves the credentials work again.
                self.login_required = false;

                debug_assert!(
                    self.cursor.as_ref().is_some_and(|c| !c.is_empty()),
                    "a successful sync should always return a cursor",
                );

                // Served before written: the request in hand should not wait
                // on a database round trip for data the server is already
                // holding.
                actions.push(Action::ToClient {
                    action: ActionClient::Serve,
                });

                // One Item today. The fetch already names which, so when
                // there is a second this follows the id on the event rather
                // than reaching for the first connection.
                if let Some(connection) = connections.first() {
                    actions.push(Action::ToStore {
                        action: ActionStore::SaveFetched {
                            item_id: connection.id.clone(),
                            institution_name: Some(connection.name.clone()),
                            added: added.clone(),
                            modified: modified.clone(),
                            removed: removed.clone(),
                            accounts: connection.accounts.clone(),
                            streams: streams.clone(),
                            cursor: cursor.clone(),
                        },
                    });
                }
            }

            EventSource::PlaidFailed { code } => {
                if code == "ITEM_LOGIN_REQUIRED" {
                    self.login_required = true;

                    actions.push(Action::ToProcess {
                        action: ActionProcess::ReportLoginRequired,
                    });

                    // Remembered across a restart, or the server comes back up
                    // and asks Plaid the same question it was just refused.
                    if let Some(connection) = self.connections.first() {
                        actions.push(Action::ToStore {
                            action: ActionStore::SaveStatus {
                                item_id: connection.id.clone(),
                                error_code: Some(code.clone()),
                            },
                        });
                    }
                }

                // A failed fetch still counts as an attempt, so a Plaid outage
                // does not turn into a request loop against it.
                self.fetched_at = Some(self.now);

                // Whatever is held is better than nothing, and on the very
                // first fetch that is an empty list, which the client draws as
                // an empty state rather than an error.
                actions.push(Action::ToClient {
                    action: ActionClient::Serve,
                });
            }
        }
    }

    fn handle_event_process(&mut self, event: &EventProcess, actions: &mut Vec<Action>) {
        match event {
            // Read before fetching. Whatever is on disk is what the last run
            // ended with, and starting from it is the difference between a
            // restart costing nothing and a restart re-downloading a history.
            EventProcess::Started => {
                actions.push(Action::ToStore {
                    action: ActionStore::LoadEverything,
                });
            }
        }
    }

    fn handle_event_store(&mut self, event: &EventStore, _actions: &mut Vec<Action>) {
        match event {
            EventStore::Loaded {
                entries,
                connections,
                streams,
                cursor,
                synced_at,
            } => {
                // Assigned, not applied — unlike a Plaid sync. This is not a
                // diff against what is held; it *is* what is held, read back.
                // It only ever arrives once, before anything else has run.
                debug_assert!(
                    self.ledger.entries().is_empty(),
                    "the store is read once, before anything is fetched",
                );

                self.ledger = Ledger::new(entries.clone());
                self.connections = connections.clone();
                self.streams = streams.clone();
                self.cursor = cursor.clone();

                // The stored time, not now: data read from disk is exactly as
                // old as it was when it was written, and pretending otherwise
                // would hold off the first fetch for a freshness window the
                // data has already spent.
                self.fetched_at = *synced_at;

                self.login_required = connections
                    .iter()
                    .any(|c| matches!(c.status, ConnectionStatus::LoginRequired));

                tracing::info!(
                    entries = entries.len(),
                    connections = connections.len(),
                    streams = streams.len(),
                    "read back from the store",
                );
            }

            // Both of these carry on. Nothing to decide: an empty ledger is
            // what a first run has, and the next fetch fills it. They are
            // events rather than a line in the runtime because the log is
            // meant to be a recording of the session, and "the store was
            // unreachable" is part of the story.
            EventStore::LoadFailed { message } => {
                tracing::error!(message, "could not read the store; starting empty");
            }

            EventStore::SaveFailed { message } => {
                tracing::error!(message, "could not write the store");
            }
        }
    }

    fn handle_event_terminal(&mut self, event: &EventTerminal, actions: &mut Vec<Action>) {
        match event {
            // Both mean stop, and they mean it the same way here — the
            // difference matters to whoever sent it, not to us. They are
            // separate variants so the log says which arrived, and so a future
            // arm (SIGTERM draining longer, say) does not need a rename.
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

    /// Whether to go back to Plaid.
    ///
    /// Pure, and reads the clock it was handed rather than one of its own —
    /// the whole reason this lives in a state machine.
    fn should_fetch(&mut self, item_id: &str, force: bool) -> bool {
        // Nothing new starts once we are on the way out.
        if self.shutting_down {
            return false;
        }

        // No amount of asking fixes a login, so this outranks even `force`.
        if self.login_required {
            return false;
        }

        let stale = match self.fetched_at {
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

    pub fn entries(&self) -> &[Entry] {
        self.ledger.entries()
    }

    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    pub fn streams(&self) -> &[Stream] {
        &self.streams
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
    /// `handle_events`, from the runtime that read it. It cannot be
    /// `#[cfg(test)]` because the tests are a separate crate, which sees this
    /// one built as an ordinary dependency.
    pub fn set_now(&mut self, now: Timestamp) {
        self.now = now;
    }

    pub fn login_required(&self) -> bool {
        self.login_required
    }

    pub fn shutting_down(&self) -> bool {
        self.shutting_down
    }
}
