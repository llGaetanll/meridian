//! The server's state machine, tested the way `docs/STYLE.md` says: events in
//! with `<`, actions out with `>`.
//!
//! What these pin is when the server decides to go back to Plaid — the two
//! things that cost real money to get wrong are its per-endpoint rate limit
//! and retrying an Item whose login has expired — and that a sync's diff is
//! handed to the store rather than applied anywhere in here.
//!
//! The ledger itself is no longer the machine's, so it is no longer tested
//! here. `/transactions/sync` being a diff is now `src-postgres`'s
//! `apply_entries` to get right, and its tests are the ones that pin it.

use api_common::Timestamp;
use api_eval::eval;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Tx;

use server_main::*;

const T0: Timestamp = Timestamp::from_secs(1_787_097_600);

const ITEM: &str = "item-1";

fn state() -> StateServer {
    let mut state = StateServer::new();
    state.set_now(T0);
    state
}

fn entry(id: &str) -> Entry {
    Entry::from_tx(
        id.to_string(),
        Tx::Transfer {
            ts: Timestamp::from_secs(1_787_000_000),
            pos: Position {
                asset: "USD".into(),
                qty: -1.0,
            },
        },
    )
}

/// A bank as the store holds it, ready to be decided about.
fn facts(synced_at: Option<Timestamp>, login_required: bool) -> ItemFacts {
    ItemFacts {
        item_id: ITEM.to_string(),
        cursor: synced_at.map(|_| "cursor-1".to_string()),
        synced_at,
        login_required,
    }
}

fn requested(force: bool, items: Vec<ItemFacts>) -> Event {
    Event::FromRouter {
        event: EventRouter::SnapshotRequested { force, items },
    }
}

fn loaded(added: Vec<Entry>) -> Event {
    Event::FromSource {
        event: EventSource::PlaidLoaded {
            item_id: ITEM.to_string(),
            added,
            modified: vec![],
            removed: vec![],
            connection: None,
            streams: vec![],
            cursor: "cursor-2".into(),
        },
    }
}

fn failed(code: &str) -> Event {
    Event::FromSource {
        event: EventSource::PlaidFailed {
            item_id: ITEM.to_string(),
            code: code.into(),
        },
    }
}

/// A bank that has never synced is asked about immediately.
#[test]
fn a_bank_that_has_never_synced_is_fetched() {
    let mut state = state();

    eval!(&mut state,
        < requested(false, vec![facts(None, false)]),
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: None, .. } },
        > Action::ToRouter { action: ActionRouter::Serve },
    );
}

/// A user with no banks is served rather than fetched for. Not an error: it is
/// what "no bank linked yet" looks like, and the state first run exists to
/// leave.
#[test]
fn a_user_with_no_banks_is_just_served() {
    let mut state = state();

    eval!(&mut state,
        // Forced, which normally outranks everything short of a dead login.
        < requested(true, vec![]),
        > Action::ToRouter { action: ActionRouter::Serve },
    );
}

#[test]
fn a_fresh_bank_is_served_without_asking_plaid_again() {
    let mut state = state();

    eval!(&mut state,
        < requested(false, vec![facts(Some(T0), false)]),
        > Action::ToRouter { action: ActionRouter::Serve },
    );
}

/// Staleness is measured against the clock handed in, not one the machine
/// reads — which is what makes this a test rather than a wait.
#[test]
fn a_stale_bank_is_refetched_from_its_cursor() {
    let mut state = state();

    let long_ago = Timestamp::from_secs(T0.as_secs() - FRESH_FOR_SECS);

    eval!(&mut state,
        < requested(false, vec![facts(Some(long_ago), false)]),
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: Some(_), .. } },
        > Action::ToRouter { action: ActionRouter::Serve },
    );
}

#[test]
fn a_forced_request_refetches_however_fresh() {
    let mut state = state();

    eval!(&mut state,
        < requested(true, vec![facts(Some(T0), false)]),
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: Some(_), .. } },
        > Action::ToRouter { action: ActionRouter::Serve },
    );
}

/// Retrying a login-required Item cannot help, and Plaid counts the call.
#[test]
fn a_login_required_bank_is_not_retried_even_when_forced() {
    let mut state = state();

    eval!(&mut state,
        < requested(true, vec![facts(Some(T0), true)]),
        > Action::ToRouter { action: ActionRouter::Serve },
    );
}

/// Every bank the user has is decided about separately, so one dead login does
/// not stop the others syncing.
#[test]
fn each_bank_is_decided_about_on_its_own() {
    let mut state = state();

    let dead = ItemFacts {
        item_id: "item-2".to_string(),
        cursor: None,
        synced_at: None,
        login_required: true,
    };

    eval!(&mut state,
        < requested(false, vec![facts(None, false), dead]),
        > Action::ToSource { action: ActionSource::FetchPlaid { cursor: None, .. } },
        > Action::ToRouter { action: ActionRouter::Serve },
    );
}

/// A sync goes to the store and nowhere else. The machine holds no ledger to
/// apply a diff to, which is what retired the trap in `docs/PLAID.md` rather
/// than guarding against it.
#[test]
fn a_sync_is_handed_to_the_store() {
    let mut state = state();

    eval!(&mut state,
        < loaded(vec![entry("a")]),
        > Action::ToStore { action: ActionStore::SaveFetched { .. } },
    );
}

/// An expired login has to outlive the process, or the server comes back up
/// and asks Plaid the same question it was just refused.
#[test]
fn an_expired_login_is_reported_and_written_down() {
    let mut state = state();

    eval!(&mut state,
        < failed("ITEM_LOGIN_REQUIRED"),
        > Action::ToProcess { action: ActionProcess::ReportLoginRequired },
        > Action::ToStore { action: ActionStore::SaveStatus { .. } },
    );
}

/// Any other failure is just a failure: nothing to write down, and the next
/// request will try again.
#[test]
fn an_ordinary_failure_writes_nothing() {
    let mut state = state();

    eval!(&mut state,
        < failed("INTERNAL_SERVER_ERROR"),
    );
}

/// Plaid meters per endpoint; a burst has to be refused rather than sent and
/// rejected, because a rejected call still counts against the budget.
#[test]
fn a_burst_of_forced_refreshes_is_rate_limited() {
    let mut state = state();
    let mut actions = Vec::new();

    // BalanceGet allows 10/min, so the 11th forced refresh has no budget.
    let mut fetches = 0;

    for _ in 0..12 {
        actions.clear();
        state.handle_events(
            T0,
            &[requested(true, vec![facts(Some(T0), false)])],
            &mut actions,
        );

        if actions.iter().any(|a| matches!(a, Action::ToSource { .. })) {
            fetches += 1;
        }
    }

    assert_eq!(fetches, 10, "the endpoint's per-minute budget");
}

/// The budget is per Item, so a second bank is not refused because the first
/// one spent its allowance.
#[test]
fn the_rate_limit_is_per_bank() {
    let mut state = state();
    let mut actions = Vec::new();

    for _ in 0..12 {
        actions.clear();
        state.handle_events(
            T0,
            &[requested(true, vec![facts(Some(T0), false)])],
            &mut actions,
        );
    }

    let second = ItemFacts {
        item_id: "item-2".to_string(),
        cursor: None,
        synced_at: None,
        login_required: false,
    };

    actions.clear();
    state.handle_events(T0, &[requested(true, vec![second])], &mut actions);

    assert!(
        actions.iter().any(|a| matches!(a, Action::ToSource { .. })),
        "a second bank has its own budget",
    );
}

#[test]
fn shutting_down_stops_new_work() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromTerminal { event: EventTerminal::SigInt },
        > Action::ToProcess { action: ActionProcess::Shutdown },

        < requested(true, vec![facts(None, false)]),
        > Action::ToRouter { action: ActionRouter::Serve },
    );

    assert!(state.shutting_down());
}

/// A second signal while the first is still draining must not queue a second
/// shutdown.
#[test]
fn a_second_signal_is_ignored() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromTerminal { event: EventTerminal::SigInt },
        < Event::FromTerminal { event: EventTerminal::SigTerm },
        > Action::ToProcess { action: ActionProcess::Shutdown },
    );
}

/// Entropy arrives from the edge, so the machine has a generator without ever
/// reading one. A fixed seed here is what makes the codes below assertable.
fn seeded() -> Event {
    Event::FromProcess {
        event: EventProcess::Seeded { seed: [7; 32] },
    }
}

fn code_requested(email: &str) -> Event {
    Event::FromRouter {
        event: EventRouter::CodeRequested {
            email: email.to_string(),
        },
    }
}

fn code_submitted(email: &str, code: &str, stored: Option<StoredCode>) -> Event {
    Event::FromRouter {
        event: EventRouter::CodeSubmitted {
            email: email.to_string(),
            code: code.to_string(),
            stored,
        },
    }
}

fn stored_code(code: &str, attempts: i32, expires_in: u64) -> StoredCode {
    StoredCode {
        code_sha256: server_main::hash_code(code),
        expires_at: Timestamp::from_secs(T0.as_secs() + expires_in),
        attempts,
    }
}

/// Asking for a code stores its hash and sends the code, in that order and as
/// two actions: only one of them may ever hold the plaintext.
#[test]
fn asking_for_a_code_stores_a_hash_and_sends_the_code() {
    let mut state = state();

    eval!(&mut state,
        < seeded(),

        < code_requested("alex@example.com"),
        > Action::ToStore { action: ActionStore::SaveCode { .. } },
        > Action::ToEmail { action: ActionEmail::SendCode { .. } },
        > Action::ToRouter { action: ActionRouter::Accepted },
    );
}

/// The whole reason the generator is in here: the same seed produces the same
/// code, so an event log replays to the value it produced the first time.
#[test]
fn the_same_seed_produces_the_same_code() {
    let code_from = |seed: u8| {
        let mut state = state();
        let mut actions = Vec::new();

        state.handle_events(
            T0,
            &[
                Event::FromProcess {
                    event: EventProcess::Seeded { seed: [seed; 32] },
                },
                code_requested("alex@example.com"),
            ],
            &mut actions,
        );

        actions
            .iter()
            .find_map(|a| match a {
                Action::ToEmail {
                    action: ActionEmail::SendCode { code, .. },
                } => Some(code.clone()),
                _ => None,
            })
            .expect("a code was sent")
    };

    assert_eq!(code_from(7), code_from(7), "a replay lands where it did");
    assert_ne!(code_from(7), code_from(8), "and a different seed does not");
}

/// No entropy, no credentials. A predictable sign-in code is a way into
/// somebody's bank, so an unseeded machine refuses rather than filling in
/// anything derivable — and this is the test that says nothing is stored and
/// nothing is sent.
///
/// It answers rather than going quiet. A route parks until its own action
/// comes back, so a refusal with no action is a request that never returns.
#[test]
fn an_unseeded_machine_will_not_mint_a_code() {
    let mut state = state();

    eval!(&mut state,
        < code_requested("alex@example.com"),
        > Action::ToRouter { action: ActionRouter::Unavailable },
    );
}

#[test]
fn the_right_code_signs_you_in_and_spends_itself() {
    let mut state = state();

    eval!(&mut state,
        < seeded(),

        < code_submitted("alex@example.com", "123456", Some(stored_code("123456", 0, 600))),
        > Action::ToStore { action: ActionStore::CreateSession { .. } },
        > Action::ToStore { action: ActionStore::DeleteCode { .. } },
        > Action::ToRouter { action: ActionRouter::SignedIn { .. } },
    );
}

/// A wrong code costs a guess and nothing else — no session, and the code is
/// still there to be got right.
#[test]
fn a_wrong_code_only_counts_a_guess() {
    let mut state = state();

    eval!(&mut state,
        < seeded(),

        < code_submitted("alex@example.com", "654321", Some(stored_code("123456", 0, 600))),
        > Action::ToStore { action: ActionStore::RecordAttempt { .. } },
        > Action::ToRouter { action: ActionRouter::Rejected },
    );
}

/// An expired code counts a guess too. An attacker who could reset the cap by
/// waiting would have an unlimited number of tries at the next one.
#[test]
fn an_expired_code_is_rejected_and_still_counts() {
    let mut state = state();

    eval!(&mut state,
        < seeded(),

        < code_submitted("alex@example.com", "123456", Some(stored_code("123456", 0, 0))),
        > Action::ToStore { action: ActionStore::RecordAttempt { .. } },
        > Action::ToRouter { action: ActionRouter::Rejected },
    );
}

/// Out of guesses outranks a right answer, or the cap would be advice.
#[test]
fn an_exhausted_code_rejects_the_right_answer() {
    let mut state = state();

    eval!(&mut state,
        < seeded(),

        < code_submitted("alex@example.com", "123456", Some(stored_code("123456", 5, 600))),
        > Action::ToStore { action: ActionStore::RecordAttempt { .. } },
        > Action::ToRouter { action: ActionRouter::Rejected },
    );
}

/// No code standing against the address. Rejected like any other, and
/// deliberately not distinguishable — the difference would say which addresses
/// have a sign-in in flight.
#[test]
fn an_address_with_no_code_is_rejected_the_same_way() {
    let mut state = state();

    eval!(&mut state,
        < seeded(),

        < code_submitted("alex@example.com", "123456", None),
        > Action::ToRouter { action: ActionRouter::Rejected },
    );
}

const USER: uuid::Uuid = uuid::Uuid::nil();

fn link_polled(finished: bool, tokens: Vec<&str>) -> Event {
    Event::FromSource {
        event: EventSource::LinksPolled {
            polls: vec![server_main::LinkPoll {
                link_token: "link-1".to_string(),
                finished,
                public_tokens: tokens.into_iter().map(str::to_string).collect(),
            }],
        },
    }
}

/// Asking to connect a bank opens a session and nothing else. Nothing is
/// created at Plaid's end, so there is nothing yet to write down.
#[test]
fn connecting_a_bank_opens_a_link_session() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { event: EventRouter::LinkRequested { user_id: USER } },
        > Action::ToSource { action: ActionSource::OpenLink { .. } },
    );
}

/// The session is written down *before* its URL is handed over. The other
/// order leaves a window in which a session could be finished that we have no
/// record of asking for — which is what stops one account collecting a bank
/// another account connected.
#[test]
fn a_session_is_written_down_before_its_url_is_handed_over() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromSource {
            event: EventSource::LinkOpened {
                user_id: USER,
                link_token: "link-1".to_string(),
                hosted_link_url: "https://secure.plaid.com/x".to_string(),
            },
        },
        > Action::ToStore { action: ActionStore::SaveLink { .. } },
        > Action::ToRouter { action: ActionRouter::LinkOpened { .. } },
    );
}

/// A token is the answer whether or not the session says it has finished:
/// somebody partway through adding a second bank has produced a real Item
/// already.
#[test]
fn a_public_token_is_exchanged_even_before_the_session_finishes() {
    let mut state = state();

    eval!(&mut state,
        < link_polled(false, vec!["public-1"]),
        > Action::ToSource { action: ActionSource::ExchangePublicToken { .. } },
    );
}

/// Finished with nothing: they closed the sheet. An outcome, not a failure,
/// and the session is marked done so it is not polled forever.
#[test]
fn a_session_that_ends_with_nothing_is_collected_and_reported() {
    let mut state = state();

    eval!(&mut state,
        < link_polled(true, vec![]),
        > Action::ToStore { action: ActionStore::CollectLink { .. } },
        > Action::ToRouter { action: ActionRouter::NotLinked },
    );
}

/// Still open — somebody is still typing their password. Left pending, so the
/// next close asks again rather than giving up on a bank they did connect.
#[test]
fn an_open_session_is_left_pending() {
    let mut state = state();

    eval!(&mut state,
        < link_polled(false, vec![]),
        > Action::ToRouter { action: ActionRouter::NotLinked },
    );
}

/// The bank is stored and the session closed, in that order.
#[test]
fn a_linked_bank_is_stored_and_its_session_closed() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromSource {
            event: EventSource::ItemLinked {
                link_token: "link-1".to_string(),
                item_id: "item-9".to_string(),
            },
        },
        > Action::ToStore { action: ActionStore::SaveItem { .. } },
        > Action::ToStore { action: ActionStore::CollectLink { .. } },
        > Action::ToRouter { action: ActionRouter::Linked },
    );
}

/// Closing the sheet with nothing open asks Plaid nothing. There is no session
/// to collect, so there is no call worth making.
#[test]
fn closing_a_sheet_with_no_session_open_asks_nothing() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { event: EventRouter::LinkClosed { pending: vec![] } },
        > Action::ToRouter { action: ActionRouter::NotLinked },
    );
}

/// Every session the user has open is asked about, so somebody who backed out
/// of one bank and connected another does not lose the one that worked.
#[test]
fn every_open_session_is_polled() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter {
            event: EventRouter::LinkClosed {
                pending: vec!["link-1".to_string(), "link-2".to_string()],
            },
        },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },
    );
}

/// One question, one answer. The route can only send one response, so however
/// many sessions were asked about, exactly one action comes back for it — the
/// invariant the whole request/reply pairing rests on.
#[test]
fn many_sessions_still_produce_one_answer() {
    let mut state = state();

    let polled = |tokens: Vec<&str>| Event::FromSource {
        event: EventSource::LinksPolled {
            polls: tokens
                .into_iter()
                .map(|link_token| server_main::LinkPoll {
                    link_token: link_token.to_string(),
                    finished: true,
                    public_tokens: Vec::new(),
                })
                .collect(),
        },
    };

    eval!(&mut state,
        < polled(vec!["link-1", "link-2", "link-3"]),
        > Action::ToStore { action: ActionStore::CollectLink { .. } },
        > Action::ToStore { action: ActionStore::CollectLink { .. } },
        > Action::ToStore { action: ActionStore::CollectLink { .. } },
        > Action::ToRouter { action: ActionRouter::NotLinked },
    );
}
