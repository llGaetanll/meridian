//! Flow F of `docs/API_FLOW.md` — the webhooks that arrive for the life of an
//! Item — tested the way `docs/STYLE.md` says: events in with `<`, actions out
//! with `>`.
//!
//! Nothing here has to do with linking. `SYNC_UPDATES_AVAILABLE` starts a
//! `/transactions/sync` nobody asked for and nobody waits on;
//! `ITEM_LOGIN_REQUIRED` is written to the item's row so a restart does not
//! cheerfully ask Plaid the same question and get the same refusal. Without
//! either, we learn both late.
//!
//! They need `PLAID_COLLECT=webhook` only in the sense that they need an
//! address Plaid can reach, so every machine here is
//! `state_collecting_by_webhook()`. Both are answered exactly as C2's step 7
//! answers a webhook, and what makes one believed at all is
//! `state_webhook_trust.rs`.

mod fixtures;

use api_eval::eval;
use fixtures::T0;
use fixtures::signed;
use fixtures::state_collecting_by_webhook;
use fixtures::webhook_jwk;

use server_main::*;

/// F1. Plaid saying there is something new is not a freshness question, so a
/// bank synced a second ago is asked about anyway — and asked from its cursor,
/// which is the difference between a diff and a re-read of the whole history.
///
/// Nothing is parked on the fetch: a first sync of a long history takes longer
/// than Plaid waits for its 2xx, and a webhook Plaid decides went unanswered
/// is one it sends again.
#[test]
fn a_sync_webhook_fetches_however_fresh_the_bank_is() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"TRANSACTIONS\",\"webhook_code\":\"SYNC_UPDATES_AVAILABLE\",\"item_id\":\"item-1\"}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"TRANSACTIONS\",\"webhook_code\":\"SYNC_UPDATES_AVAILABLE\",\"item_id\":\"item-1\"}", T0.as_secs()).1 } },
        > Action::ToSource { action: ActionSource::FetchWebhookKey { call_id: 1, .. } },

        < Event::FromSource { event: EventSource::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToStore { action: ActionStore::LoadItem { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemLoaded { call_id: 2, item: Some(ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: false }) } },
        > @r#"ToSource { action: FetchPlaid { call_id: 3, item_id: "item-1", cursor: Some("cursor-1") } }"#,
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// No amount of asking fixes a login, and a webhook does not change that.
#[test]
fn a_sync_webhook_does_not_retry_a_dead_login() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"TRANSACTIONS\",\"webhook_code\":\"SYNC_UPDATES_AVAILABLE\",\"item_id\":\"item-1\"}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"TRANSACTIONS\",\"webhook_code\":\"SYNC_UPDATES_AVAILABLE\",\"item_id\":\"item-1\"}", T0.as_secs()).1 } },
        > Action::ToSource { action: ActionSource::FetchWebhookKey { call_id: 1, .. } },

        < Event::FromSource { event: EventSource::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToStore { action: ActionStore::LoadItem { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemLoaded { call_id: 2, item: Some(ItemFacts { item_id: "item-1".to_string(), cursor: Some("cursor-1".to_string()), synced_at: Some(T0), login_required: true }) } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// Plaid still has news about a bank the store has no row for. There is
/// nothing to sync against, and the webhook is taken anyway — refusing it
/// would only make Plaid send it again.
#[test]
fn a_sync_webhook_for_a_bank_we_do_not_hold_is_still_taken() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"TRANSACTIONS\",\"webhook_code\":\"SYNC_UPDATES_AVAILABLE\",\"item_id\":\"item-gone\"}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"TRANSACTIONS\",\"webhook_code\":\"SYNC_UPDATES_AVAILABLE\",\"item_id\":\"item-gone\"}", T0.as_secs()).1 } },
        > Action::ToSource { action: ActionSource::FetchWebhookKey { call_id: 1, .. } },

        < Event::FromSource { event: EventSource::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToStore { action: ActionStore::LoadItem { call_id: 2, .. } },

        < Event::FromStore { event: EventStore::ItemLoaded { call_id: 2, item: None } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// F2. The write is what makes it survive a restart: a server that came back
/// up having forgotten would ask Plaid the same question and be refused the
/// same way. It is also what the Reconnect button reads.
#[test]
fn a_dead_login_webhook_is_written_down() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"ITEM\",\"webhook_code\":\"ITEM_LOGIN_REQUIRED\",\"item_id\":\"item-1\"}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"ITEM\",\"webhook_code\":\"ITEM_LOGIN_REQUIRED\",\"item_id\":\"item-1\"}", T0.as_secs()).1 } },
        > Action::ToSource { action: ActionSource::FetchWebhookKey { call_id: 1, .. } },

        < Event::FromSource { event: EventSource::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToProcess { action: ActionProcess::ReportLoginRequired },
        > @r#"ToStore { action: SaveStatus { call_id: 2, item_id: "item-1", error_code: Some("ITEM_LOGIN_REQUIRED") } }"#,

        // 7a. Taken only once the row is down, for the same reason C2's is:
        // a failure before the write is a webhook Plaid sends again.
        < Event::FromStore { event: EventStore::WriteSuccess { call_id: 2 } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}

/// They signed in again. The only thing that clears the flag without a sync
/// being tried, which is what makes the connection usable again immediately.
#[test]
fn a_repaired_login_webhook_clears_it() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"ITEM\",\"webhook_code\":\"LOGIN_REPAIRED\",\"item_id\":\"item-1\"}", T0.as_secs()).1 } },
        > Action::ToSource { action: ActionSource::FetchWebhookKey { call_id: 1, .. } },

        < Event::FromSource { event: EventSource::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > @r#"ToStore { action: SaveStatus { call_id: 2, item_id: "item-1", error_code: None } }"#,
    );
}

/// Plaid sends types nobody planned for and retries anything not answered 2xx,
/// so an unfamiliar one is taken and nothing is done about it.
#[test]
fn an_unknown_webhook_is_taken_and_acted_on_by_nothing() {
    let mut state = state_collecting_by_webhook();

    eval!(&mut state,
        < Event::FromRouter { request_id: 1, client: Client { ip: ClientIp::new("203.0.113.7"), user: None }, event: EventRouter::WebhookReceived { jwt: signed("{\"webhook_type\":\"ASSETS\",\"webhook_code\":\"PRODUCT_READY\"}", T0.as_secs()).0, body: signed("{\"webhook_type\":\"ASSETS\",\"webhook_code\":\"PRODUCT_READY\"}", T0.as_secs()).1 } },
        > Action::ToSource { action: ActionSource::FetchWebhookKey { call_id: 1, .. } },

        < Event::FromSource { event: EventSource::WebhookKeyLoaded { call_id: 1, kid: "kid-1".to_string(), key: Some(webhook_jwk()) } },
        > Action::ToRouter { action: ActionRouter::Accepted, .. },
    );
}
