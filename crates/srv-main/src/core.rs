//! The server's own decisions, kept away from its IO.
//!
//! Same shape as `api-core` and for the same reason (`docs/ARCHITECTURE.md`):
//! events in, actions out, nothing in between touching the outside world. The
//! server has fewer decisions than the app, but the ones it has are exactly
//! the kind worth being able to replay — when to go back to Plaid, what to do
//! with an `ITEM_LOGIN_REQUIRED`, whether a cursor is still good.
//!
//! What this buys concretely: the "should I refetch?" rule below is a pure
//! function of state and a clock reading handed in, so a stale-cache bug is
//! reproducible from a log rather than from a stopwatch.

use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Ledger;
use api_ledger::Stream;

/// How long a snapshot is served before Plaid is asked again.
///
/// Plaid rate-limits `/accounts/balance/get` hard, and a phone that refetched
/// on every page load would hit it. Thirty seconds is short enough that a
/// pull-to-refresh feels live and long enough that flicking between tabs
/// costs nothing.
pub const FRESH_FOR_SECS: u64 = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum ServerEvent {
    /// A client asked for the current snapshot.
    SnapshotRequested { now: u64, force: bool },

    /// Plaid answered.
    ///
    /// The transaction lists are a **diff** — what changed since the cursor —
    /// so they are applied to the held ledger rather than replacing it. A
    /// second sync a minute later legitimately returns three empty lists.
    PlaidLoaded {
        now: u64,
        added: Vec<Entry>,
        modified: Vec<Entry>,
        removed: Vec<String>,
        connections: Vec<Connection>,
        streams: Vec<Stream>,
        cursor: String,
    },

    /// Plaid did not answer, or answered with an error.
    PlaidFailed { now: u64, code: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerAction {
    /// Go and ask Plaid. Carries the cursor to resume from, if there is one.
    FetchPlaid { cursor: Option<String> },

    /// Answer the client from what is already held.
    Serve,

    /// This connection needs the user to sign in again; nothing else fixes it.
    MarkLoginRequired,
}

/// Everything the server holds between requests.
#[derive(Debug, Default)]
pub struct ServerState {
    /// Everything synced so far. Held across fetches, because each fetch only
    /// says what changed.
    ledger: Ledger,

    connections: Vec<Connection>,
    streams: Vec<Stream>,

    /// Where the last sync got to. The only record of how far we read, so it
    /// is kept even when a later fetch fails.
    cursor: Option<String>,

    /// When the held data was last refreshed. `None` means never.
    fetched_at: Option<u64>,

    /// Set once Plaid says the Item needs a new login. Sticky: retrying is
    /// pointless until the user acts, so the server stops asking.
    login_required: bool,
}

impl ServerState {
    pub fn handle_events(&mut self, events: &[ServerEvent], actions: &mut Vec<ServerAction>) {
        for event in events {
            self.handle_event(event, actions);
        }
    }

    fn handle_event(&mut self, event: &ServerEvent, actions: &mut Vec<ServerAction>) {
        match event {
            ServerEvent::SnapshotRequested { now, force } => {
                if self.should_fetch(*now, *force) {
                    actions.push(ServerAction::FetchPlaid {
                        cursor: self.cursor.clone(),
                    });
                } else {
                    actions.push(ServerAction::Serve);
                }
            }

            ServerEvent::PlaidLoaded {
                now,
                added,
                modified,
                removed,
                connections,
                streams,
                cursor,
            } => {
                // Applied, not assigned. This is the difference between a
                // ledger that accumulates and one that empties itself on the
                // second sync.
                self.ledger.apply(added.clone(), modified.clone(), removed);

                self.connections = connections.clone();
                self.streams = streams.clone();
                self.cursor = Some(cursor.clone());
                self.fetched_at = Some(*now);

                // A successful fetch is the only thing that clears this: it
                // proves the credentials work again.
                self.login_required = false;

                actions.push(ServerAction::Serve);
            }

            ServerEvent::PlaidFailed { now, code } => {
                if code == "ITEM_LOGIN_REQUIRED" {
                    self.login_required = true;
                    actions.push(ServerAction::MarkLoginRequired);
                }

                // A failed fetch still counts as an attempt, so a Plaid outage
                // does not turn into a request loop against it.
                self.fetched_at = Some(*now);

                // Whatever is held is better than nothing, and on the very
                // first fetch that is an empty list, which the client draws as
                // an empty state rather than an error.
                actions.push(ServerAction::Serve);
            }
        }
    }

    /// Whether to go back to Plaid.
    ///
    /// Pure, and takes the clock as an argument rather than reading one — the
    /// whole reason this lives in a state machine.
    fn should_fetch(&self, now: u64, force: bool) -> bool {
        // No amount of asking fixes a login, so this outranks even `force`.
        if self.login_required {
            return false;
        }

        if force {
            return true;
        }

        match self.fetched_at {
            None => true,
            Some(at) => now.saturating_sub(at) >= FRESH_FOR_SECS,
        }
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

    pub fn login_required(&self) -> bool {
        self.login_required
    }

    pub fn has_data(&self) -> bool {
        self.fetched_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(state: &mut ServerState, events: &[ServerEvent]) -> Vec<ServerAction> {
        let mut actions = Vec::new();
        state.handle_events(events, &mut actions);
        actions
    }

    fn loaded(now: u64) -> ServerEvent {
        ServerEvent::PlaidLoaded {
            now,
            added: vec![],
            modified: vec![],
            removed: vec![],
            connections: vec![],
            streams: vec![],
            cursor: "cursor-1".into(),
        }
    }

    fn entry(id: &str) -> Entry {
        Entry::from_tx(
            id.to_string(),
            api_ledger::Tx::Transfer {
                ts: 1_787_000_000,
                pos: api_ledger::Position {
                    asset: "USD".into(),
                    qty: -1.0,
                },
            },
        )
    }

    /// The trap that emptied the app on its second fetch.
    ///
    /// `/transactions/sync` is a diff: once the cursor is saved, a sync a
    /// minute later legitimately answers with nothing new. Assigning that
    /// answer over the held ledger wipes it, and the screen goes blank while
    /// every log line still says the sync succeeded.
    #[test]
    fn a_second_sync_with_nothing_new_keeps_the_ledger() {
        let mut state = ServerState::default();

        run(
            &mut state,
            &[ServerEvent::PlaidLoaded {
                now: 1_000,
                added: vec![entry("a"), entry("b")],
                modified: vec![],
                removed: vec![],
                connections: vec![],
                streams: vec![],
                cursor: "cursor-1".into(),
            }],
        );

        assert_eq!(state.entries().len(), 2);

        // The empty second page, exactly as Plaid sends it.
        run(&mut state, &[loaded(2_000)]);

        assert_eq!(state.entries().len(), 2, "the ledger must not empty itself");
    }

    #[test]
    fn a_later_sync_adds_to_what_is_held() {
        let mut state = ServerState::default();

        run(
            &mut state,
            &[ServerEvent::PlaidLoaded {
                now: 1_000,
                added: vec![entry("a")],
                modified: vec![],
                removed: vec![],
                connections: vec![],
                streams: vec![],
                cursor: "c1".into(),
            }],
        );

        run(
            &mut state,
            &[ServerEvent::PlaidLoaded {
                now: 2_000,
                added: vec![entry("b")],
                modified: vec![],
                removed: vec![entry("a").id],
                connections: vec![],
                streams: vec![],
                cursor: "c2".into(),
            }],
        );

        let ids: Vec<&str> = state.entries().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["b"], "added joins, removed leaves");
    }

    #[test]
    fn the_first_request_goes_to_plaid() {
        let mut state = ServerState::default();

        let actions = run(
            &mut state,
            &[ServerEvent::SnapshotRequested {
                now: 1_000,
                force: false,
            }],
        );

        assert_eq!(actions, vec![ServerAction::FetchPlaid { cursor: None }]);
    }

    #[test]
    fn a_fresh_snapshot_is_served_without_asking_plaid_again() {
        let mut state = ServerState::default();

        run(&mut state, &[loaded(1_000)]);

        let actions = run(
            &mut state,
            &[ServerEvent::SnapshotRequested {
                now: 1_010,
                force: false,
            }],
        );

        assert_eq!(actions, vec![ServerAction::Serve]);
    }

    #[test]
    fn a_stale_snapshot_is_refetched() {
        let mut state = ServerState::default();

        run(&mut state, &[loaded(1_000)]);

        let actions = run(
            &mut state,
            &[ServerEvent::SnapshotRequested {
                now: 1_000 + FRESH_FOR_SECS,
                force: false,
            }],
        );

        assert_eq!(
            actions,
            vec![ServerAction::FetchPlaid {
                cursor: Some("cursor-1".into())
            }]
        );
    }

    #[test]
    fn a_forced_request_refetches_however_fresh() {
        let mut state = ServerState::default();

        run(&mut state, &[loaded(1_000)]);

        let actions = run(
            &mut state,
            &[ServerEvent::SnapshotRequested {
                now: 1_001,
                force: true,
            }],
        );

        assert!(matches!(actions[0], ServerAction::FetchPlaid { .. }));
    }

    /// Retrying a login-required Item cannot help, and Plaid counts the call.
    #[test]
    fn a_login_required_item_is_not_retried_even_when_forced() {
        let mut state = ServerState::default();

        run(
            &mut state,
            &[ServerEvent::PlaidFailed {
                now: 1_000,
                code: "ITEM_LOGIN_REQUIRED".into(),
            }],
        );

        assert!(state.login_required());

        let actions = run(
            &mut state,
            &[ServerEvent::SnapshotRequested {
                now: 9_999,
                force: true,
            }],
        );

        assert_eq!(actions, vec![ServerAction::Serve]);
    }

    #[test]
    fn a_successful_fetch_clears_login_required() {
        let mut state = ServerState::default();

        run(
            &mut state,
            &[ServerEvent::PlaidFailed {
                now: 1_000,
                code: "ITEM_LOGIN_REQUIRED".into(),
            }],
        );
        run(&mut state, &[loaded(2_000)]);

        assert!(!state.login_required());
    }

    /// An outage must not turn into a hot loop against Plaid.
    #[test]
    fn a_transient_failure_still_counts_as_an_attempt() {
        let mut state = ServerState::default();

        run(
            &mut state,
            &[ServerEvent::PlaidFailed {
                now: 1_000,
                code: "INTERNAL_SERVER_ERROR".into(),
            }],
        );

        let actions = run(
            &mut state,
            &[ServerEvent::SnapshotRequested {
                now: 1_005,
                force: false,
            }],
        );

        assert_eq!(actions, vec![ServerAction::Serve]);
        assert!(!state.login_required(), "only a login error is sticky");
    }

    #[test]
    fn the_same_events_give_the_same_actions() {
        let events = [
            ServerEvent::SnapshotRequested {
                now: 1_000,
                force: false,
            },
            loaded(1_001),
            ServerEvent::SnapshotRequested {
                now: 1_002,
                force: false,
            },
        ];

        let first = run(&mut ServerState::default(), &events);
        let second = run(&mut ServerState::default(), &events);

        assert_eq!(first, second);
    }
}
