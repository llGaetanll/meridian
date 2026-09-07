//! Flow C1 of `docs/API_FLOW.md` — collecting a Link session by polling — the
//! way `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! `PLAID_COLLECT=poll` is what a laptop runs, because polling needs no
//! address of our own. Here `/link/finish` is a *trigger*: nothing else ever
//! collects, so that request is what goes and asks Plaid, and a sheet the user
//! dismissed never gets here at all.
//!
//! Every machine in this file is `state()`, which is the polling one. The same
//! close on a machine Plaid calls is `flow_c2_collecting_by_webhook.rs`, and
//! the two are independent: the last test here is the one that says so.
//!
//! C1's 6c and 6d — no session, too many closes — are `flow_e_the_ledger.rs`
//! and `state_limits.rs`, which pin them once for every authenticated route.

mod fixtures;

use api_eval::eval;
use fixtures::state;

use server_main::*;

/// C1. The whole of it: a sheet closes, Plaid is asked what it came to, and
/// the one token it produced is traded for a bank — steps 1 through 6a.
///
/// The route is answered as soon as the bank is attached, not when its history
/// has arrived. Those are different questions and the first sync is the long
/// one, so the fetch goes out on a call nobody is parked on: the two writes
/// release the route while it is still outstanding.
#[test]
fn a_closed_sheet_attaches_the_bank_and_starts_its_sync() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        // 2. One call for every session this user has open, because the
        // request does not say which one closed.
        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        // 3a and 4. The irreversible call: in Production this spends an Item
        // slot that removing the Item does not give back.
        < Event::FromSource { event: EventSource::LinksPolled { call_id: 3, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: true, public_tokens: vec!["public-1".to_string()] }] } },
        > Action::ToSource { action: ActionSource::ExchangePublicToken { call_id: 4, .. } },

        // 5. The bank is written down and the session is closed in the same
        // turn.
        < Event::FromSource { event: EventSource::ItemLinked { call_id: 4, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 5, .. } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 6, .. } },

        // D starts on the write reporting back, because that write is what
        // resolves whose bank it is — and on a call nobody is parked on, or
        // this route stays open for the whole of the first sync.
        < Event::FromStore { call_id: 5, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }) } },
        > @r#"ToSource { action: FetchBalances { call_id: 7, item_id: "item-9", now: Timestamp { t: 1787097600 } } }"#,

        // 6a. Both writes down, and the fetch still out. The count, not a
        // name: nothing knows what the bank is called yet.
        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: Linked { count: 1 } }"#,
    );
}

/// A token is the answer whether or not the session says it has finished:
/// somebody partway through adding a second bank has produced a real Item
/// already.
#[test]
fn a_public_token_is_exchanged_even_before_the_session_finishes() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 3, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: false, public_tokens: vec!["public-1".to_string()] }] } },
        > Action::ToSource { action: ActionSource::ExchangePublicToken { .. } },
    );
}

/// One close attaches one bank, and the note under C1 in `docs/API_FLOW.md`
/// says what that costs: the first `public_token` found is exchanged, and the
/// session is marked collected in the same turn, so a second Item made inside
/// the *same* session has no later close that can reach it. Its token is
/// dropped.
///
/// The route can only be told about one, which is why it works this way. What
/// the claim is about is the second `ExchangePublicToken` that does not come
/// out — a bank somebody connected and does not get.
#[test]
fn a_second_bank_in_one_session_has_its_token_dropped() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 3, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: true, public_tokens: vec!["public-1".to_string(), "public-2".to_string()] }] } },
        > @r#"ToSource { action: ExchangePublicToken { call_id: 4, link_token: "link-1", public_token: "public-1" } }"#,

        < Event::FromSource { event: EventSource::ItemLinked { call_id: 4, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 5, .. } },
        > @r#"ToStore { action: CollectLink { call_id: 6, link_token: "link-1" } }"#,

        < Event::FromStore { call_id: 5, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }) } },
        > Action::ToSource { action: ActionSource::FetchBalances { call_id: 7, .. } },
    );
}

/// C1 3b and 6b. Finished with nothing: they closed the sheet. An outcome, not
/// a failure, and the session is marked done so it is not polled forever.
#[test]
fn a_session_that_ends_with_nothing_is_collected_and_reported() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 3, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: true, public_tokens: vec![] }] } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 4, .. } },

        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },
        > Action::ToRouter { action: ActionRouter::LinkAbandoned, .. },
    );
}

/// C1 3c. Still open — somebody is still typing their password. Left pending,
/// so the next close asks again rather than giving up on a bank they did
/// connect. The route hears the same sentence as 6b, which is the honest limit
/// of polling and the reason C2 exists.
#[test]
fn an_open_session_is_left_pending() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        // No `CollectLink`: an unfinished session stays standing.
        < Event::FromSource { event: EventSource::LinksPolled { call_id: 3, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: false, public_tokens: vec![] }] } },
        > Action::ToRouter { action: ActionRouter::LinkAbandoned, .. },
    );
}

/// Closing the sheet with nothing open asks Plaid nothing. There is no session
/// to collect, so there is no call worth making.
#[test]
fn closing_a_sheet_with_no_session_open_asks_nothing() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec![] } },
        > Action::ToRouter { action: ActionRouter::LinkAbandoned, .. },
    );
}

/// Every session the user has open goes into one call — so somebody who backed
/// out of one bank and connected another does not lose the one that worked —
/// and however many were asked about, exactly one action comes back for the
/// route. That is the invariant the whole request/reply pairing rests on: the
/// route can only send one response.
#[test]
fn every_open_session_is_polled_and_the_route_still_answers_once() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string(), "link-2".to_string(), "link-3".to_string()] } },
        > @r#"ToSource { action: PollLinks { call_id: 3, link_tokens: ["link-1", "link-2", "link-3"] } }"#,

        < Event::FromSource { event: EventSource::LinksPolled { call_id: 3, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: true, public_tokens: vec![] }, LinkPoll { link_token: "link-2".to_string(), finished: true, public_tokens: vec![] }, LinkPoll { link_token: "link-3".to_string(), finished: true, public_tokens: vec![] }] } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 4, .. } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 5, .. } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 6, .. } },

        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },

        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },

        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },
        > Action::ToRouter { action: ActionRouter::LinkAbandoned, .. },
    );
}

/// C1 6e. This user's open sessions could not be read, so there is nothing to
/// poll and nothing true to say about the sheet they just closed.
#[test]
fn sessions_that_cannot_be_read_cannot_be_collected() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::ReadError { message: "connection refused".to_string() } },
        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,
    );
}

/// The laptop cannot receive a webhook, so polling has to be untouched by
/// everything C2 added: the same close, on a machine told to poll, still asks
/// Plaid rather than reading an outcome that will never be written.
#[test]
fn a_closed_sheet_still_polls_when_we_collect_by_polling() {
    let mut state = state();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },
        > Action::ToSource { action: ActionSource::PollLinks { .. } },
    );
}

#[test]
fn a_bank_the_store_refused_is_written_on_the_retry() {
    let mut state = state();

    eval!(&mut state,
        // The sheet closed.
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        // Who is asking.
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        // Them.
        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        // What they have open.
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        // One session.
        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },
        // Ask Plaid what it came to.
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        // A token to trade.
        < Event::FromSource { event: EventSource::LinksPolled { call_id: 3, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: true, public_tokens: vec!["public-1".to_string()] }] } },
        // The call that spends the slot.
        > Action::ToSource { action: ActionSource::ExchangePublicToken { call_id: 4, .. } },

        // The bank exists at Plaid from here on.
        < Event::FromSource { event: EventSource::ItemLinked { call_id: 4, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        // Write it down.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 5, .. } },
        // And close the session.
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 6, .. } },

        // The session is closed. Nothing is released: the answer waits on the
        // write, not on this.
        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },

        // The write is not.
        < Event::FromStore { call_id: 5, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // Ask again. The access token is still in the runtime's map, so the
        // same write can land; five attempts is the bound.
        > @r#"ToStore { action: SaveItem { call_id: 7, item_id: "item-9", link_token: "link-1" } }"#,

        // The row exists.
        < Event::FromStore { call_id: 7, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }) } },
        // Start its first sync.
        > Action::ToSource { action: ActionSource::FetchBalances { call_id: 8, .. } },
        // And say the one sentence that is now true.
        > @r#"ToRouter { request_id: 1, action: Linked { count: 1 } }"#,
    );
}

#[test]
fn a_bank_the_store_would_not_write_is_answered_an_error() {
    let mut state = state();

    eval!(&mut state,
        // The sheet closed.
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        // Who is asking.
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        // Them.
        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        // What they have open.
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        // One session.
        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },
        // Ask Plaid what it came to.
        > Action::ToSource { action: ActionSource::PollLinks { .. } },

        // A token to trade.
        < Event::FromSource { event: EventSource::LinksPolled { call_id: 3, polls: vec![LinkPoll { link_token: "link-1".to_string(), finished: true, public_tokens: vec!["public-1".to_string()] }] } },
        // The call that spends the slot.
        > Action::ToSource { action: ActionSource::ExchangePublicToken { call_id: 4, .. } },

        // The bank exists at Plaid from here on.
        < Event::FromSource { event: EventSource::ItemLinked { call_id: 4, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        // Write it down.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 5, .. } },
        // And close the session.
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 6, .. } },

        // The session is closed.
        < Event::FromStore { call_id: 6, event: EventStore::WriteSuccess },

        // The first attempt.
        < Event::FromStore { call_id: 5, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // The second.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 7, .. } },

        < Event::FromStore { call_id: 7, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // The third.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 8, .. } },

        < Event::FromStore { call_id: 8, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // The fourth.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 9, .. } },

        < Event::FromStore { call_id: 9, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // The fifth, and the last: a store that has refused this five times is
        // down rather than busy, and the route is holding all the while.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 10, .. } },

        // `ItemSaved { item: None }` ends here too, and without the four
        // retries: nothing to attach does not become something to attach.
        < Event::FromStore { call_id: 10, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // A 500. Not `LinkAbandoned`, which is a sentence about somebody's
        // bank: this bank is live at Plaid and nothing here owns it. No
        // `FetchBalances` either, because there is no row to sync.
        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,
    );
}
