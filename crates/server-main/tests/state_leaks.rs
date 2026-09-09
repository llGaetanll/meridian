//! What the machine is still holding once a flow is over.
//!
//! Every other test in this directory reads the actions a flow produced, and
//! no action says how many entries are left in the maps behind it. A server is
//! one process for weeks: a request that fills a map and does not empty it is
//! a flow that passes every test here and grows until it is restarted.
//!
//! So this file makes one claim, in the only way it can be made — a
//! [`CensusServer`] taken before a flow and another taken after it, with the
//! flow written out end to end in between.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Tx;
use fixtures::T0;
use fixtures::signed;
use fixtures::state_collecting_by_webhook;
use fixtures::webhook_jwk;

use server_main::*;

/// A whole first run, and then the launch after it: a bank connected by
/// webhook, the close that collects it, ten minutes of nothing, and a ledger
/// served from a bank gone stale.
///
/// Five of the machine's tables are filled and emptied along the way — the
/// calls in flight, the routes in flight at whatever step they reached, the
/// outcome Plaid reported before anybody asked, the fetch that was running,
/// the windows both callers were metered in — and the claim is that all of it
/// is given back. The signing key is the one thing deliberately kept, and it
/// is the one thing this assertion excuses.
#[test]
fn a_first_run_and_the_launch_after_it_give_back_everything_they_took() {
    let mut state = state_collecting_by_webhook();

    let before = state.census();

    eval!(&mut state,
        // Plaid says a Link session finished with a token to exchange. Nothing
        // is believed until the key it names has been fetched.
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\"]}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\"]}", T0.as_secs()).1 } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchWebhookKey { call_id: 1, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::ExchangePublicToken { call_id: 2, .. } },

        // The bank is written down. Nobody has asked what the session came
        // to yet, so the outcome waits for a route that will.
        < Event::FromSourcePlaid { event: EventSourcePlaid::ItemLinked { call_id: 2, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 3, .. } },

        // The write says whose bank it is, which is what the first sync is
        // started from — on a call nobody is parked on, so the webhook is
        // answered without waiting for it.
        < Event::FromStore { call_id: 3, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }) } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchBalances { call_id: 4, .. } },
        > @r#"ToRouter { request_id: 1, action: Accepted }"#,

        // The phone catches up and is told what its session came to, which is
        // also what collects the outcome the webhook left behind.
        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 5, .. } },

        < Event::FromStore { call_id: 5, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 6, .. } },

        < Event::FromStore { call_id: 6, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 7, .. } },

        < Event::FromStore { call_id: 7, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 2, action: Linked { count: 1 } }"#,

        // The first sync runs its three calls out. Nobody is waiting on it,
        // so its last write is the end of it.
        < Event::FromSourcePlaid { event: EventSourcePlaid::BalancesLoaded { call_id: 4, item_id: "item-9".to_string(), connection: Connection { id: "item-9".to_string(), name: "First Platypus Bank".to_string(), initials: "FP".to_string(), status: api_ledger::ConnectionStatus::Ok { synced_at: T0 }, accounts: vec![] } } },
        > Action::ToStore { action: ActionStore::SaveAccounts { call_id: 8, .. } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::SyncTransactions { call_id: 9, .. } },

        < Event::FromStore { call_id: 8, event: EventStore::WriteSuccess },

        < Event::FromSourcePlaid { event: EventSourcePlaid::TransactionsLoaded { call_id: 9, item_id: "item-9".to_string(), added: vec![Entry::from_tx("a".to_string(), Tx::Transfer { ts: Timestamp::from_secs(1_787_000_000), pos: Position { asset: "USD".into(), qty: -1.0 } })], modified: vec![], removed: vec![], cursor: "cursor-2".to_string(), has_more: false } },
        > Action::ToStore { action: ActionStore::SaveTransactions { call_id: 10, .. } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchRecurring { call_id: 11, .. } },

        < Event::FromStore { call_id: 10, event: EventStore::WriteSuccess },

        < Event::FromSourcePlaid { event: EventSourcePlaid::RecurringLoaded { call_id: 11, item_id: "item-9".to_string(), streams: vec![] } },
        > Action::ToStore { action: ActionStore::SaveStreams { call_id: 12, .. } },

        < Event::FromStore { call_id: 12, event: EventStore::WriteSuccess },

        // Ten minutes in which nobody calls. Long enough for every window the
        // close was metered in to have gone idle.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + 600) } },

        // The launch after it. The bank was synced ten minutes ago, so this
        // one goes back to Plaid before it serves anything.
        < Event::FromRouter { request_id: 3, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 13, .. } },

        < Event::FromStore { call_id: 13, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadItems { call_id: 14, .. } },

        < Event::FromStore { call_id: 14, event: EventStore::ItemsLoaded { items: vec![ItemFacts { item_id: "item-9".to_string(), user_id: uuid::Uuid::nil(), cursor: Some("cursor-2".to_string()), synced_at: Some(T0), login_required: false }] } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchBalances { call_id: 15, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::BalancesLoaded { call_id: 15, item_id: "item-9".to_string(), connection: Connection { id: "item-9".to_string(), name: "First Platypus Bank".to_string(), initials: "FP".to_string(), status: api_ledger::ConnectionStatus::Ok { synced_at: Timestamp::from_secs(T0.as_secs() + 600) }, accounts: vec![] } } },
        > Action::ToStore { action: ActionStore::SaveAccounts { call_id: 16, .. } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::SyncTransactions { call_id: 17, .. } },

        < Event::FromStore { call_id: 16, event: EventStore::WriteSuccess },

        < Event::FromSourcePlaid { event: EventSourcePlaid::TransactionsLoaded { call_id: 17, item_id: "item-9".to_string(), added: vec![Entry::from_tx("a".to_string(), Tx::Transfer { ts: Timestamp::from_secs(1_787_000_000), pos: Position { asset: "USD".into(), qty: -1.0 } })], modified: vec![], removed: vec![], cursor: "cursor-3".to_string(), has_more: false } },
        > Action::ToStore { action: ActionStore::SaveTransactions { call_id: 18, .. } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchRecurring { call_id: 19, .. } },

        < Event::FromStore { call_id: 18, event: EventStore::WriteSuccess },

        < Event::FromSourcePlaid { event: EventSourcePlaid::RecurringLoaded { call_id: 19, item_id: "item-9".to_string(), streams: vec![] } },
        > Action::ToStore { action: ActionStore::SaveStreams { call_id: 20, .. } },

        < Event::FromStore { call_id: 20, event: EventStore::WriteSuccess },
        > Action::ToStore { action: ActionStore::LoadSnapshot { call_id: 21, .. } },

        < Event::FromStore { call_id: 21, event: EventStore::SnapshotLoaded { snapshot: WireSnapshot::default() } },
        > Action::ToRouter { request_id: 3, action: ActionRouter::Serve { .. } },

        // And another quiet minute, so the launch's own windows — this
        // server's and Plaid's both — go idle too.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + 700) } },
    );

    // A finished sync is what a phone watching one reads to say "You're set."
    // and the totals under it, so it stands after the fetch is over. There is
    // no rule yet for when it stops being worth keeping, which is why it is
    // the one entry this flow does not give back.
    assert_eq!(
        CensusServer {
            keys: 0,
            syncing: 0,
            ..state.census()
        },
        before,
        "everything the flow took is back, bar the key and the finished sync",
    );

    assert_eq!(state.census().keys, 1, "the key plaid signed with is kept");
}

/// A request whose read never answers is given up on, and what it was doing is
/// let go of with it.
///
/// The sharp version of the same question: a store that goes quiet is the one
/// way into this machine that leaves an entry nothing else ever removes, and
/// on the webhook route it is a stranger's body being parked there.
#[test]
fn a_call_that_is_given_up_on_takes_what_it_was_doing_with_it() {
    let mut state = state_collecting_by_webhook();

    let before = state.census();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::SnapshotRequested { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        // The store says nothing at all. Time passes and the route is
        // answered with what a machine that cannot read has to say.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + CALL_GOOD_FOR_SECS + 1) } },
        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,

        // And a minute in which the window it was metered in goes idle.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + CALL_GOOD_FOR_SECS + 62) } },
    );

    assert_eq!(state.census(), before, "the request left nothing behind");
}

/// A route parked on a Link session Plaid never reports on is let go of whole:
/// the session it watched, and the step it was parked at.
///
/// The one step a route can be at with no call out, so the tick that gives up
/// on calls never reaches it; the tick that gives up on sessions has to take
/// both halves with it.
#[test]
fn a_route_parked_on_a_session_nobody_reports_on_is_let_go_of_whole() {
    let mut state = state_collecting_by_webhook();

    let before = state.census();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        // Nothing recorded, so this parks: no call is out, and no answer.
        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + LINK_ANSWER_GOOD_FOR_SECS) } },
        > @r#"ToRouter { request_id: 1, action: LinkPending }"#,

        // And a minute in which the window it was metered in goes idle.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + LINK_ANSWER_GOOD_FOR_SECS + 62) } },
    );

    assert_eq!(state.census(), before, "the route left nothing behind");
}
