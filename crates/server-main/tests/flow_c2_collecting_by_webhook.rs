//! Flow C2 of `docs/API_FLOW.md` — collecting a Link session by webhook — the
//! way `docs/STYLE.md` says: events in with `<`, actions out with `>`.
//!
//! `PLAID_COLLECT=webhook` is what a deployed server prefers, and it needs an
//! address Plaid can reach. Collection happens unprompted, so `/link/finish`
//! stops being a trigger and becomes a *question*, one that can be asked a
//! fraction of a second before the answer exists. The doc's steps 1 and 2 race
//! and usually 2 wins; both orders have a test here, and they are the first
//! two.
//!
//! Every machine in this file is `state_subscribed_by_webhook()`. What makes a
//! webhook believed at all — the algorithm, the key, the replay window, the
//! body hash — is `state_webhook_trust.rs`, which is C2's steps 3, 4 and 7b.

mod fixtures;

use api_common::Timestamp;
use api_eval::eval;
use fixtures::T0;
use fixtures::signed;
use fixtures::state_subscribed_by_webhook;
use fixtures::webhook_jwk;

use server_main::*;

/// C2, the common order: Plaid's POST wins the race, because the phone has a
/// redirect, an app switch and a request of its own to get through first. The
/// outcome waits, and the close collects it — which is also when the session
/// is marked done.
///
/// The doc's 2 through 7a, then 1 and 8a.
#[test]
fn an_outcome_that_arrives_first_is_waiting_when_the_phone_asks() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\"]}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\"]}", T0.as_secs()).1 } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchWebhookKey { call_id: 1, .. } },

        // 5. The same irreversible call C1 makes at its own step 4.
        < Event::FromSourcePlaid { event: EventSourcePlaid::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::ExchangePublicToken { call_id: 2, .. } },

        // No `CollectLink`: the session has to stay standing, because a
        // collected one is a session `/link/finish` can no longer see.
        < Event::FromSourcePlaid { event: EventSourcePlaid::ItemLinked { call_id: 2, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 3, .. } },

        // 7a. Plaid is owed nothing beyond "taken", and it is owed it only
        // once the exchange has landed and been written — which is what makes
        // a failure mid-exchange a webhook Plaid sends again rather than a
        // bank quietly lost. D starts on the same event, because that write is
        // what resolves whose bank it is.
        < Event::FromStore { call_id: 3, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }) } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchBalances { call_id: 4, .. } },
        > @r#"ToRouter { request_id: 1, action: Accepted }"#,

        // 1 and 8a. The phone catches up, and is told what it came to.
        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 5, .. } },

        < Event::FromStore { call_id: 5, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 6, .. } },

        < Event::FromStore { call_id: 6, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 7, .. } },

        < Event::FromStore { call_id: 7, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 2, action: Linked { count: 1 } }"#,
    );
}

/// The other order, which is the one the whole design is for: the phone asks
/// before Plaid has said anything, and is parked rather than guessed at.
#[test]
fn a_close_that_arrives_first_waits_for_the_outcome() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        // Nothing at all comes back. Not an empty answer — no answer yet.
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },

        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\"]}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\"]}", T0.as_secs()).1 } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchWebhookKey { call_id: 3, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::WebhookKeyLoaded { call_id: 3, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::ExchangePublicToken { call_id: 4, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::ItemLinked { call_id: 4, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 5, .. } },

        // The write is what the session came to, so it is the write that
        // reaches the parked route and closes the session.
        < Event::FromStore { call_id: 5, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }) } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchBalances { call_id: 6, .. } },
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 7, .. } },
        > @r#"ToRouter { request_id: 2, action: Accepted }"#,

        // 8a. The route that was waiting, answered once the session it is
        // being told about has actually been closed.
        < Event::FromStore { call_id: 7, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: Linked { count: 1 } }"#,
    );
}

/// C2 5. One webhook can carry two banks, and every one of them is exchanged:
/// Plaid says this once, so a token nobody trades is a bank somebody connected
/// and does not get. There is no second call to collect the one left behind,
/// which is the difference from C1.
#[test]
fn every_token_a_session_produced_is_exchanged() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\", \"public-2\"]}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\", \"public-2\"]}", T0.as_secs()).1 } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchWebhookKey { call_id: 1, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > @r#"ToSourcePlaid { action: ExchangePublicToken { call_id: 2, link_token: "link-1", public_token: "public-1" } }"#,
        > @r#"ToSourcePlaid { action: ExchangePublicToken { call_id: 3, link_token: "link-1", public_token: "public-2" } }"#,
    );
}

/// C2 8b. They closed the sheet. Plaid says so in as many words, which is the
/// change: "no bank" used to be inferred from a poll that came back empty.
#[test]
fn a_session_that_ends_with_nothing_is_said_rather_than_inferred() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },

        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[]}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[]}", T0.as_secs()).1 } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchWebhookKey { call_id: 3, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::WebhookKeyLoaded { call_id: 3, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > @r#"ToRouter { request_id: 2, action: Accepted }"#,
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 4, .. } },

        < Event::FromStore { call_id: 4, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: LinkAbandoned }"#,
    );
}

/// An exchange that failed is nothing coming of the session, and the route
/// waiting hears that rather than waiting out the clock.
#[test]
fn an_exchange_that_fails_settles_the_session_too() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },

        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\"]}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\"]}", T0.as_secs()).1 } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchWebhookKey { call_id: 3, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::WebhookKeyLoaded { call_id: 3, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::ExchangePublicToken { call_id: 4, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::LinkAbandoned { call_id: 4, link_token: "link-1".to_string() } },
        > @r#"ToRouter { request_id: 2, action: Accepted }"#,
        > Action::ToStore { action: ActionStore::CollectLink { call_id: 5, .. } },

        < Event::FromStore { call_id: 5, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 1, action: LinkAbandoned }"#,
    );
}

/// C2 8c. Plaid never said. Not a failure — the bank may still be landing — so
/// the answer is neither of the other two, and the screen it sends somebody to
/// is the one that polls and will see a late bank arrive.
///
/// One route, two sessions, still one answer: the route can only send one
/// response, so a second session timing out must not produce a second.
#[test]
fn a_session_nobody_reports_on_is_answered_pending_exactly_once() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string(), "link-2".to_string()] } },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + LINK_ANSWER_GOOD_FOR_SECS) } },
        > @r#"ToRouter { request_id: 1, action: LinkPending }"#,
    );
}

/// On a dismissal the phone never calls `/link/finish` at all, so the outcome
/// written down for it is never collected. It goes on its own rather than
/// staying for the life of the process.
#[test]
fn an_outcome_nobody_comes_back_for_expires() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[]}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[]}", T0.as_secs()).1 } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchWebhookKey { call_id: 1, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + OUTCOME_GOOD_FOR_SECS) } },

        // Nothing recorded any more, so this parks rather than being answered
        // from a record about a sheet closed ten minutes ago.
        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 3, .. } },

        < Event::FromStore { call_id: 3, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },

        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + OUTCOME_GOOD_FOR_SECS + LINK_ANSWER_GOOD_FOR_SECS) } },
        > Action::ToRouter { action: ActionRouter::LinkPending, .. },
    );
}

/// A caller with nothing open cannot be told anything else: there is no
/// session to hear about, so `Pending` would be a promise nothing can keep.
#[test]
fn a_close_with_no_session_open_is_answered_at_once() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 1, .. } },

        < Event::FromStore { call_id: 1, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 2, .. } },

        < Event::FromStore { call_id: 2, event: EventStore::PendingLinksLoaded { link_tokens: vec![] } },
        > @r#"ToRouter { request_id: 1, action: LinkAbandoned }"#,
    );
}

/// Somebody who backed out of one bank and connected another has two
/// sessions, and the one that worked is the answer to give.
#[test]
fn the_session_that_attached_a_bank_is_the_one_reported() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[]}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[]}", T0.as_secs()).1 } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchWebhookKey { call_id: 1, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },

        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-2\",\"public_tokens\":[\"public-2\"]}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-2\",\"public_tokens\":[\"public-2\"]}", T0.as_secs()).1 } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::ExchangePublicToken { call_id: 2, .. } },

        < Event::FromSourcePlaid { event: EventSourcePlaid::ItemLinked { call_id: 2, link_token: "link-2".to_string(), item_id: "item-9".to_string() } },
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 3, .. } },

        < Event::FromStore { call_id: 3, event: EventStore::ItemSaved { item: Some(ItemFacts { item_id: "item-9".to_string(), user_id: uuid::Uuid::nil(), cursor: None, synced_at: None, login_required: false }) } },
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchBalances { call_id: 4, .. } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },

        < Event::FromRouter { request_id: 3, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 5, .. } },

        < Event::FromStore { call_id: 5, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 6, .. } },

        < Event::FromStore { call_id: 6, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string(), "link-2".to_string()] } },
        > @r#"ToStore { action: CollectLink { call_id: 7, link_token: "link-2" } }"#,

        < Event::FromStore { call_id: 7, event: EventStore::WriteSuccess },
        > @r#"ToRouter { request_id: 3, action: Linked { count: 1 } }"#,
    );
}

#[test]
fn a_bank_the_store_would_not_write_is_not_taken_from_plaid() {
    let mut state = state_subscribed_by_webhook();

    eval!(&mut state,
        // Plaid says the session is finished, with a token on it.
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\"]}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"LINK\",\"webhook_code\":\"SESSION_FINISHED\",\"link_token\":\"link-1\",\"public_tokens\":[\"public-1\"]}", T0.as_secs()).1 } },
        // The key the signature names.
        > Action::ToSourcePlaid { action: ActionSourcePlaid::FetchWebhookKey { call_id: 1, .. } },

        // Believed.
        < Event::FromSourcePlaid { event: EventSourcePlaid::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        // The call that spends the slot.
        > Action::ToSourcePlaid { action: ActionSourcePlaid::ExchangePublicToken { call_id: 2, .. } },

        // The bank exists at Plaid from here on.
        < Event::FromSourcePlaid { event: EventSourcePlaid::ItemLinked { call_id: 2, link_token: "link-1".to_string(), item_id: "item-9".to_string() } },
        // Write it down. No `CollectLink`: the session stands until
        // `/link/finish` reports it.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 3, .. } },

        // The first attempt.
        < Event::FromStore { call_id: 3, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // The second.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 4, .. } },

        < Event::FromStore { call_id: 4, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // The third.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 5, .. } },

        < Event::FromStore { call_id: 5, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // The fourth.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 6, .. } },

        < Event::FromStore { call_id: 6, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // The fifth, and the last.
        > Action::ToStore { action: ActionStore::SaveItem { call_id: 7, .. } },

        < Event::FromStore { call_id: 7, event: EventStore::WriteError { message: "connection refused".to_string() } },
        // A 503, so Plaid sends the webhook again. `Accepted` is a 204 meaning
        // taken, and taken is what stops it. The session is left unsettled,
        // which is what the close below then reads.
        > @r#"ToRouter { request_id: 1, action: Unavailable }"#,

        // The phone catches up.
        < Event::FromRouter { request_id: 2, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::LinkClosed { token_sha256: hash_token("session-1") } },
        // Who is asking.
        > Action::ToStore { action: ActionStore::LoadSession { call_id: 8, .. } },

        // Them.
        < Event::FromStore { call_id: 8, event: EventStore::SessionResolved { user_id: Some(uuid::Uuid::nil()) } },
        // What they have open.
        > Action::ToStore { action: ActionStore::LoadPendingLinks { call_id: 9, .. } },

        // One session, with no outcome recorded against it, so the route parks.
        < Event::FromStore { call_id: 9, event: EventStore::PendingLinksLoaded { link_tokens: vec!["link-1".to_string()] } },

        // Ten seconds of nobody saying anything.
        < Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + LINK_ANSWER_GOOD_FOR_SECS) } },
        // Neither `Linked` nor `Nothing`, both of which are false: the bank is
        // live at Plaid, and Plaid's re-send may yet be the turn that writes it.
        > @r#"ToRouter { request_id: 2, action: LinkPending }"#,
    );
}
