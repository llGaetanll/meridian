//! The server's own decisions, kept away from its IO.
//!
//! Same shape as the app's machine and for the same reason
//! (`docs/ARCHITECTURE.md`): events in, actions out, nothing in between
//! touching the outside world.
//!
//! No data lives here. The machine holds what cannot be read back from
//! Postgres — how much of Plaid's rate limit has been spent, and where the
//! process is in its own life; everything else arrives on the event that needs
//! it and is served from the store.
//!
//! Signing up is a process rather than a handful of fields, so it is its own
//! machine ([`StateSignup`]); what is left here is serving somebody already
//! signed up — their ledger, their syncs, and the webhooks Plaid sends about
//! them.

use std::collections::BTreeMap;
use std::collections::HashMap;

use api_common::Timestamp;
use api_common::ratelimit::RateLimiter;
use server_wire::SyncStage;
use server_wire::WireSyncStatus;
use src_plaid::ITEM_LOGIN_REQUIRED;
use src_plaid::PlaidEndpoint;
use src_plaid::PlaidFault;
use src_plaid::PlaidJwk;
use src_plaid::PlaidRateCtx;
use src_plaid::PlaidRateLimiter;
use src_plaid::PlaidWebhook;
use src_plaid::PlaidWebhookJwt;
use src_plaid::PlaidWebhookKind;
use uuid::Uuid;

use crate::Action;
use crate::ActionProcess;
use crate::ActionRouter;
use crate::ActionSocket;
use crate::ActionSourcePlaid;
use crate::ActionStore;
use crate::Authenticated;
use crate::CachedKey;
use crate::CallId;
use crate::CensusServer;
use crate::Client;
use crate::ClientIp;
use crate::ConnId;
use crate::Entitlement;
use crate::Event;
use crate::EventProcess;
use crate::EventRouter;
use crate::EventSocket;
use crate::EventSourcePlaid;
use crate::EventSourceStripe;
use crate::EventStore;
use crate::EventTerminal;
use crate::ItemFacts;
use crate::Lifecycle;
use crate::Owed;
use crate::Pending;
use crate::RequestId;
use crate::StateBilling;
use crate::StateSignup;
use crate::Syncing;
use crate::WebhookFacts;
use crate::rate_limit::ServerRateCtx;
use crate::rate_limit::ServerRateLimiter;

/// How long a sync is good for before Plaid is asked again. Plaid
/// rate-limits `/accounts/balance/get` hard, and thirty seconds is short
/// enough that a pull-to-refresh still feels live.
pub const FRESH_FOR_SECS: u64 = 30;

/// How long a `kid` Plaid would not answer for is remembered as a miss.
///
/// Short, because the two reasons for a miss want opposite things: an invented
/// `kid` should cost one call however often it is sent, and a real key we
/// failed to fetch must not be refused for the life of the process.
pub const WEBHOOK_KEY_MISS_GOOD_FOR_SECS: u64 = 5 * 60;

/// How many streams one user may hold open at once.
///
/// A rule rather than a socket count, so it is arithmetic the machine does and
/// an `eval!` test can reach. Generous for a person — a phone, a laptop and
/// room to spare — and small enough that a client reconnecting without letting
/// go cannot collect connections for the life of the process.
pub const WATCHERS_AT_MOST: usize = 10;

/// How many webhook signing keys are remembered at once. Finite because the
/// ids come off tokens strangers send: full means the misses are swept, and
/// still full means the webhook is refused.
pub const WEBHOOK_KEYS_AT_MOST: usize = 64;

/// Everything the server holds between requests.
///
/// Fields are private: state changes only through
/// [`handle_events`](StateServer::handle_events).
#[derive(Debug, Default)]
pub struct StateServer {
    state: Lifecycle,

    /// Signing somebody up, from the first code to the first bank. The calls
    /// in flight are its too — nearly all of them are its own.
    signup: StateSignup,

    /// Starting subscriptions, and hearing what came of them. Its own
    /// process, the way signing up is.
    billing: StateBilling,

    /// Who has paid, and until when. Held here rather than inside
    /// [`StateBilling`] because it is not billing's process: a link request
    /// and a snapshot both ask it, and two copies would be two copies that
    /// can disagree about whether somebody has paid.
    ///
    /// `BTreeMap` rather than a hash, per the rule about determinism: this is
    /// walked when the census counts it, and a walk in a different order is a
    /// replay that lands somewhere else.
    entitled: BTreeMap<Uuid, Entitlement>,

    /// What Plaid will let us send. A fact about this process's last minute,
    /// which is why it cannot be read back from the store.
    limiter: PlaidRateLimiter,

    /// What we will let a caller ask of us. Swept on the tick: its keys come
    /// from whoever calls us, so one that only inserted would be the thing
    /// that exhausts the server.
    requests: ServerRateLimiter,

    /// How far each running fetch has got, by item. Kept because a fetch is
    /// over by the time anything could read what it did; empty means idle.
    syncing: HashMap<String, Syncing>,

    /// Who is being written to, and on whose behalf. Keyed by the connection
    /// so that letting one go is a removal, and capped per user by
    /// [`WATCHERS_AT_MOST`].
    watchers: HashMap<ConnId, Uuid>,

    /// When the current batch is being handled.
    now: Timestamp,

    /// What is known about each webhook signing key, by the `kid` a token
    /// named. Swept on the tick: these names come from strangers.
    keys: HashMap<String, CachedKey>,
}

impl StateServer {
    pub fn new() -> Self {
        Self::default()
    }

    /// The only entry point. Time is not an argument but an event, which is
    /// what makes a log of the events a recording of the session.
    pub fn handle_events(&mut self, events: &[Event], actions: &mut Vec<Action>) {
        for event in events {
            self.handle_event(event, actions);
        }

        // Only once every event in the batch has been handled.
        self.settle();
    }

    fn handle_event(&mut self, event: &Event, actions: &mut Vec<Action>) {
        match event {
            Event::FromRouter {
                request_id,
                client,
                event,
            } => self.handle_event_router(*request_id, client, event, actions),
            Event::FromSourcePlaid { event } => self.handle_event_source(event, actions),
            Event::FromSourceStripe { event } => self.handle_event_stripe(event, actions),
            Event::FromSocket { event } => self.handle_event_socket(event),
            Event::FromStore { call_id, event } => {
                self.handle_event_store(*call_id, event, actions)
            }
            Event::FromProcess { event } => self.handle_event_process(event, actions),
            Event::FromTerminal { event } => self.handle_event_terminal(event, actions),
        }
    }

    fn handle_event_router(
        &mut self,
        request_id: RequestId,
        client: &Client,
        event: &EventRouter,
        actions: &mut Vec<Action>,
    ) {
        // A route that has to know who is asking asks the store first.
        if let Some(token_sha256) = event.token() {
            let Some(authenticated) = Authenticated::of(event) else {
                unreachable!("a variant carrying a token names what it is waiting for")
            };

            self.signup.waits_on(
                request_id,
                Pending::Authenticating {
                    ip: client.ip.clone(),
                    event: authenticated,
                },
            );

            actions.push(Action::ToStore {
                action: ActionStore::LoadSession {
                    call_id: self.signup.calling(request_id, self.now),
                    token_sha256: token_sha256.to_vec(),
                    now: self.now,
                },
            });

            return;
        }

        // Checked after the work would be a limit on how much we are thanked
        // for, not on how much we do.
        if let Some(route) = event.route() {
            // A botnet has an address each and one victim, so on the route
            // that sends mail the inbox is the window that sees one caller.
            let ctx = ServerRateCtx::new(route, client.clone()).mailing(event.recipient());

            // Where nothing can return early past it and leave a request
            // served but uncounted.
            if !self.requests.can_send(self.now, &ctx) {
                tracing::warn!(
                    client = %client.ip,
                    route = route.as_str(),
                    "a caller is over its rate limit",
                );

                self.signup
                    .answer(request_id, ActionRouter::TooMany, self.now, actions);

                return;
            }
        }

        match event {
            EventRouter::LinkRequested { .. } => {
                unreachable!("a request carrying a token is resolved before it is decided")
            }

            EventRouter::SyncWatchRequested { .. } | EventRouter::LinkClosed { .. } => {
                unreachable!("a request carrying a token is resolved before it is decided")
            }

            EventRouter::CodeRequested { email } => {
                self.signup
                    .code_requested(request_id, email, self.now, actions);
            }

            EventRouter::CodeSubmitted { email, code } => {
                self.signup
                    .code_submitted(request_id, email, code, self.now, actions);
            }

            EventRouter::WebhookReceived { jwt, body } => {
                self.believe(request_id, jwt, body, actions)
            }

            // The signature is the whole of what protects this: no session,
            // and by `EventRouter::route` no rate limit either. Believed in
            // the machine rather than at the edge, so which signature is
            // acceptable and whether it is inside its window are both rules
            // an `eval!` test can reach.
            EventRouter::StripeWebhookReceived { signature, body } => {
                self.believe_stripe(request_id, signature, body, actions)
            }

            EventRouter::SnapshotRequested { .. }
            | EventRouter::SubscribeRequested { .. }
            | EventRouter::BillingStatusRequested { .. } => {
                unreachable!("a request carrying a token is resolved before it is decided")
            }
        }
    }

    /// Whether a POST to the Stripe webhook is really Stripe's, and what to do
    /// about it if it is.
    ///
    /// **Answered in the same turn the signature checks out**, before the
    /// write lands. Stripe retries anything it does not hear about in about
    /// twenty seconds, so waiting for the store would turn one payment into a
    /// pile of duplicate deliveries. What makes that safe is the ordering
    /// guard on the write: a repeat or an out-of-order delivery cannot move
    /// the row backwards.
    fn believe_stripe(
        &mut self,
        request_id: RequestId,
        signature: &str,
        body: &[u8],
        actions: &mut Vec<Action>,
    ) {
        if !self.billing.believes(body, signature, self.now) {
            self.signup
                .answer(request_id, ActionRouter::Unauthorized, self.now, actions);

            return;
        }

        let Ok(event) = serde_json::from_slice::<src_stripe::StripeWebhookEvent>(body) else {
            tracing::warn!("a believed webhook was not an event this build understands");

            // Accepted rather than refused: the signature was ours, so this is
            // an event we did not ask for or a shape we do not read, and
            // refusing it only asks Stripe to send it again.
            self.signup
                .answer(request_id, ActionRouter::Accepted, self.now, actions);

            return;
        };

        self.signup
            .answer(request_id, ActionRouter::Accepted, self.now, actions);

        let src_stripe::StripeWebhookObject::Subscription(subscription) = &event.data.object else {
            // An invoice or anything else: it says nothing about entitlement
            // that the subscription event beside it does not say better.
            return;
        };

        let entitlement = StateBilling::entitlement_of(subscription);

        // Held only for a user this process knows about. One it does not is a
        // subscription created against another database — a test key pointed
        // at a live server, or a customer from before this table existed.
        match self.billing.user_for_subscription(&subscription.id) {
            Some(user_id) => {
                self.entitled.insert(user_id, entitlement);
            }
            None => {
                tracing::warn!(
                    subscription = subscription.id,
                    "a webhook named a subscription this process does not know"
                );
            }
        }

        // Sent whatever the lookup said. The row may exist in the store even
        // where this process has not got it in hand, and the query is keyed by
        // the subscription rather than the user.
        let call_id = self.signup.calling(request_id, self.now);

        actions.push(Action::ToStore {
            action: ActionStore::ApplySubscriptionEvent {
                call_id,
                subscription_id: subscription.id.clone(),
                status: subscription.status.clone(),
                current_period_end: subscription
                    .current_period_end()
                    .and_then(|secs| u64::try_from(secs).ok())
                    .map(Timestamp::from_secs),
                cancel_at_period_end: subscription.cancel_at_period_end,
                event_id: event.id.clone(),
                event_at: u64::try_from(event.created)
                    .map(Timestamp::from_secs)
                    .unwrap_or(self.now),
            },
        });
    }

    /// Carry on with a request now that the store has said who is asking.
    ///
    /// The rate limit is asked here rather than on arrival, because charging
    /// the address first spends a slot the user's own window may refuse.
    fn handle_authenticated(
        &mut self,
        request_id: RequestId,
        ip: ClientIp,
        event: &Authenticated,
        user_id: Option<Uuid>,
        actions: &mut Vec<Action>,
    ) {
        // A token that names nobody: nothing to charge, nothing to serve.
        let Some(user_id) = user_id else {
            self.signup
                .answer(request_id, ActionRouter::Unauthorized, self.now, actions);

            return;
        };

        let route = event.route();
        let ctx = ServerRateCtx::new(route, Client::new(ip.clone(), Some(user_id)));

        if !self.requests.can_send(self.now, &ctx) {
            tracing::warn!(
                client = %ip,
                route = route.as_str(),
                "a caller is over its rate limit",
            );

            self.signup
                .answer(request_id, ActionRouter::TooMany, self.now, actions);

            return;
        }

        // There are no unpaid users. Refused here rather than at the edge, so
        // it is a rule an `eval!` test can reach — and so the two routes that
        // lead *to* paying are the only ones a session without a subscription
        // can use.
        if event.needs_entitlement() && !self.allows(user_id) {
            tracing::info!(%user_id, route = route.as_str(), "a route was asked for without a subscription");

            self.signup
                .answer(request_id, ActionRouter::PaymentRequired, self.now, actions);

            return;
        }

        match event {
            Authenticated::LinkRequested => self.signup.link_requested(
                request_id,
                user_id,
                &mut self.limiter,
                self.now,
                actions,
            ),

            Authenticated::LinkClosed => {
                self.signup
                    .link_closing(request_id, user_id, self.now, actions);
            }

            // Nothing to read: what is running is the machine's own state, and
            // it is the only thing that knows.
            Authenticated::SyncWatchRequested { conn_id } => {
                self.handle_sync_watch(request_id, *conn_id, user_id, actions)
            }

            // The rows this route decides from, scoped to the caller: two
            // users share a process and share no cursors.
            Authenticated::SnapshotRequested => {
                self.signup.waits_on(
                    request_id,
                    Pending::Loading {
                        user_id,
                        event: *event,
                    },
                );

                actions.push(Action::ToStore {
                    action: ActionStore::LoadItems {
                        call_id: self.signup.calling(request_id, self.now),
                        user_id,
                    },
                });
            }

            // The address is what Stripe emails a receipt to, and the machine
            // does not hold one: nothing it keeps says who a user is besides
            // their id. So the store is asked, and the conversation with
            // Stripe starts a turn later.
            Authenticated::SubscribeRequested => {
                self.signup.waits_on(
                    request_id,
                    Pending::Loading {
                        user_id,
                        event: *event,
                    },
                );

                actions.push(Action::ToStore {
                    action: ActionStore::LoadUser {
                        call_id: self.signup.calling(request_id, self.now),
                        user_id,
                    },
                });
            }

            // Answered from what the machine holds. Nothing to read: the
            // entitlement is already here, and asking the store would be a
            // second copy that can disagree with it.
            Authenticated::BillingStatusRequested => {
                let entitlement = self.entitled.get(&user_id);

                self.signup.answer(
                    request_id,
                    ActionRouter::BillingStatus {
                        entitled: self.allows(user_id),
                        status: entitlement.map(|e| e.status.clone()),
                        good_until: entitlement
                            .and_then(|e| e.good_until)
                            .map(|until| until.as_secs()),
                        cancel_at_period_end: entitlement.is_some_and(|e| e.cancel_at_period_end),
                    },
                    self.now,
                    actions,
                );
            }
        }
    }

    /// Whether this user may use the app, as of this batch's clock.
    ///
    /// The date against the tick and nothing else — see
    /// [`Entitlement::allows`]. No sweep and no timer: entitlement lapses
    /// because `now` moved past a date, which is the same shape as every
    /// other waiting rule in this machine.
    fn allows(&self, user_id: Uuid) -> bool {
        self.entitled
            .get(&user_id)
            .is_some_and(|entitlement| entitlement.allows(self.now))
    }

    /// Somebody asked for their ledger. Which banks are worth going back to
    /// is decided here; the ledger is read once those calls land, so a
    /// snapshot never serves rows a fetch of its own was replacing.
    fn handle_snapshot(
        &mut self,
        request_id: RequestId,
        user_id: Uuid,
        items: &[ItemFacts],
        actions: &mut Vec<Action>,
    ) {
        let asked: Vec<_> = items
            .iter()
            // Never forced: this asks for the ledger, not a trip to the bank.
            .filter(|item| self.should_fetch(item, false))
            .map(|item| (item.item_id.clone(), item.cursor.clone()))
            .collect();

        for (item_id, cursor) in asked {
            let call_id = self.signup.calling(request_id, self.now);

            self.fetch(call_id, item_id, user_id, cursor, actions);
        }

        // Read once the last of those has landed and been written, so what is
        // served is the rows they wrote. A user with no banks asked nothing,
        // so the read goes out now and serves the empty ledger.
        self.signup
            .owe(request_id, Owed::Snapshot { user_id }, self.now, actions);
    }

    /// Somebody wants to be told how their sync goes.
    fn handle_sync_watch(
        &mut self,
        request_id: RequestId,
        conn_id: ConnId,
        user_id: Uuid,
        actions: &mut Vec<Action>,
    ) {
        let open = self
            .watchers
            .values()
            .filter(|watcher| **watcher == user_id)
            .count();

        // Refused rather than closing the oldest: a client that cannot let go
        // is a client whose oldest connection is the one still working.
        if open >= WATCHERS_AT_MOST {
            tracing::warn!(%user_id, open, "a caller is holding too many streams open");

            self.signup
                .answer(request_id, ActionRouter::TooMany, self.now, actions);

            return;
        }

        self.watchers.insert(conn_id, user_id);

        let status = self.status_for(user_id);

        self.signup.answer(
            request_id,
            ActionRouter::Watching { conn_id, status },
            self.now,
            actions,
        );
    }

    /// A stream went away.
    fn handle_event_socket(&mut self, event: &EventSocket) {
        match event {
            EventSocket::Closed { conn_id } => {
                // Absent is ordinary: one refused for the cap was never
                // written down, and the connection still closes.
                self.watchers.remove(conn_id);
            }
        }
    }

    /// How far this user's sync has got.
    ///
    /// The furthest-behind of their running fetches: the screen draws one set
    /// of rows, and the honest stage is the unfinished one.
    fn status_for(&self, user_id: Uuid) -> WireSyncStatus {
        self.syncing
            .values()
            .filter(|syncing| syncing.user_id == user_id)
            // One still running outranks one that finished at the same stage,
            // or a second bank landing would be reported as already done.
            .min_by_key(|syncing| (syncing.finished, Syncing::stage_index(syncing.stage)))
            .map_or_else(WireSyncStatus::idle, Syncing::status)
    }

    /// Tell everyone watching this user where their sync has got to.
    ///
    /// The whole status every time. A stream that carried changes would leave
    /// a client that reconnected mid-sync holding a picture it could not
    /// complete.
    fn push_status(&mut self, user_id: Uuid, actions: &mut Vec<Action>) {
        let status = self.status_for(user_id);

        for (conn_id, watcher) in &self.watchers {
            if *watcher != user_id {
                continue;
            }

            actions.push(Action::ToSocket {
                action: ActionSocket::Send {
                    conn_id: *conn_id,
                    status: status.clone(),
                },
            });
        }
    }

    /// Whether this really is Plaid, and if so what it said.
    ///
    /// Every check is a rule rather than mechanics — which algorithm is
    /// acceptable, how long an unheard-of `kid` stays refused — so it belongs
    /// here rather than at the edge.
    fn believe(
        &mut self,
        request_id: RequestId,
        jwt: &str,
        body: &[u8],
        actions: &mut Vec<Action>,
    ) {
        // Everything free before anything expensive: refusing the wrong
        // algorithm stops a made-up `kid` reaching a call to Plaid.
        let parsed = match PlaidWebhookJwt::parse(jwt) {
            Ok(parsed) => parsed,

            Err(rejection) => {
                tracing::warn!(%rejection, "a webhook was not believed");

                self.signup
                    .answer(request_id, ActionRouter::Unauthorized, self.now, actions);

                return;
            }
        };

        match self.known(parsed.kid()) {
            Some(Some(key)) => self.verified(request_id, &parsed, &key, body, actions),

            // Refused without asking Plaid again, the reason a miss is kept.
            Some(None) => {
                tracing::warn!(
                    kid = parsed.kid(),
                    "a webhook named a key plaid does not have"
                );

                self.signup
                    .answer(request_id, ActionRouter::Unauthorized, self.now, actions);
            }

            None => self.fetch_webhook_key(request_id, parsed.kid(), jwt, body, actions),
        }
    }

    /// Go and ask Plaid for a key, if there is room to remember the answer.
    /// The one place an unauthenticated request can cause an outbound call.
    fn fetch_webhook_key(
        &mut self,
        request_id: RequestId,
        kid: &str,
        jwt: &str,
        body: &[u8],
        actions: &mut Vec<Action>,
    ) {
        if self.keys.len() >= WEBHOOK_KEYS_AT_MOST {
            tracing::error!(
                kid,
                "refusing to fetch another webhook key: too many have been asked for",
            );

            self.signup
                .answer(request_id, ActionRouter::Unauthorized, self.now, actions);

            return;
        }

        // Unavailable rather than Unauthorized: this webhook may well be
        // Plaid's, and a 503 is one it sends again in a minute — by which time
        // the budget is back.
        if !self.may_call(PlaidEndpoint::WebhookKeyGet, None) {
            tracing::warn!(kid, "a webhook key was not fetched: too many this minute");

            self.signup
                .answer(request_id, ActionRouter::Unavailable, self.now, actions);

            return;
        }

        self.signup.waits_on(
            request_id,
            Pending::WebhookKey {
                jwt: jwt.to_string(),
                body: body.to_vec(),
            },
        );

        actions.push(Action::ToSourcePlaid {
            action: ActionSourcePlaid::FetchWebhookKey {
                call_id: self.signup.calling(request_id, self.now),
                kid: kid.to_string(),
            },
        });
    }

    /// The signature holds. What did it say? The body is read only now: one
    /// parsed before its hash was checked is a body a stranger wrote.
    fn verified(
        &mut self,
        request_id: RequestId,
        jwt: &PlaidWebhookJwt,
        key: &PlaidJwk,
        body: &[u8],
        actions: &mut Vec<Action>,
    ) {
        if let Err(rejection) = jwt.verify(key, body, self.now.as_secs() as i64) {
            tracing::warn!(%rejection, "a webhook was not believed");

            self.signup
                .answer(request_id, ActionRouter::Unauthorized, self.now, actions);

            return;
        }

        let webhook: PlaidWebhook = match serde_json::from_slice(body) {
            Ok(webhook) => webhook,

            Err(e) => {
                tracing::error!(error = %e, "plaid signed a body that is not JSON");

                self.signup
                    .answer(request_id, ActionRouter::Unauthorized, self.now, actions);

                return;
            }
        };

        tracing::info!(
            webhook_type = webhook.webhook_type,
            webhook_code = webhook.webhook_code,
            "a webhook arrived from plaid",
        );

        let facts = match webhook.kind() {
            PlaidWebhookKind::SessionFinished {
                link_token,
                public_tokens,
            } => WebhookFacts::SessionFinished {
                link_token,
                public_tokens,
            },

            // The one kind that needs a read: it names a bank, not a person.
            PlaidWebhookKind::SyncUpdatesAvailable { item_id } => {
                self.signup.waits_on(
                    request_id,
                    Pending::WebhookItem {
                        item_id: item_id.clone(),
                    },
                );

                actions.push(Action::ToStore {
                    action: ActionStore::LoadItem {
                        call_id: self.signup.calling(request_id, self.now),
                        item_id,
                    },
                });

                return;
            }

            PlaidWebhookKind::LoginRequired { item_id } => WebhookFacts::Login {
                item_id,
                required: true,
            },

            PlaidWebhookKind::LoginRepaired { item_id } => WebhookFacts::Login {
                item_id,
                required: false,
            },

            PlaidWebhookKind::Other => WebhookFacts::Ignored {
                webhook_type: webhook.webhook_type,
                webhook_code: webhook.webhook_code,
            },
        };

        self.handle_webhook(request_id, &facts, actions);
    }

    /// What is already known about this `kid`: `Some(Some(key))` is a hit,
    /// `Some(None)` a miss still standing, and `None` nothing known — the
    /// only one that goes on to ask Plaid.
    fn known(&self, kid: &str) -> Option<Option<PlaidJwk>> {
        let cached = self.keys.get(kid)?;

        if cached.key.is_none()
            && self.now.since(cached.at).as_secs() >= WEBHOOK_KEY_MISS_GOOD_FOR_SECS
        {
            return None;
        }

        Some(cached.key.clone())
    }

    /// Drop the misses that have expired, on the tick rather than when the
    /// next webhook arrives, so invented ids cannot keep the map full.
    fn sweep_webhook_keys(&mut self) {
        let now = self.now;

        self.keys.retain(|_, cached| {
            cached.key.is_some() || now.since(cached.at).as_secs() < WEBHOOK_KEY_MISS_GOOD_FOR_SECS
        });
    }

    /// What a webhook Plaid sent us is worth doing about. Every one of them
    /// answers 2xx, because Plaid retries anything else.
    fn handle_webhook(
        &mut self,
        request_id: RequestId,
        webhook: &WebhookFacts,
        actions: &mut Vec<Action>,
    ) {
        match webhook {
            WebhookFacts::SessionFinished {
                link_token,
                public_tokens,
            } => {
                self.signup.session_finished(
                    request_id,
                    link_token,
                    public_tokens,
                    &mut self.limiter,
                    self.now,
                    actions,
                );
            }

            WebhookFacts::SyncUpdatesAvailable { item_id, item } => {
                // News about a bank the store has no row for. Nothing to fix
                // from here, but the two sides disagreeing is worth saying.
                let Some(item) = item else {
                    tracing::warn!(item_id, "plaid has news about a bank we do not hold");

                    self.signup
                        .answer(request_id, ActionRouter::Accepted, self.now, actions);

                    return;
                };

                // Forced: the webhook *is* the news the freshness window was
                // waiting for. The rate limit and a dead login still apply.
                if self.should_fetch(item, true) {
                    // Nobody waits on it: a first sync of a long history
                    // takes far longer than Plaid waits for its 2xx.
                    let call_id = self.signup.call();

                    self.fetch(
                        call_id,
                        item.item_id.clone(),
                        item.user_id,
                        item.cursor.clone(),
                        actions,
                    );
                }

                self.signup
                    .answer(request_id, ActionRouter::Accepted, self.now, actions);
            }

            WebhookFacts::Login { item_id, required } => {
                if *required {
                    actions.push(Action::ToProcess {
                        action: ActionProcess::ReportLoginRequired,
                    });
                }

                // Written down, or the server comes back up and asks Plaid
                // the same question it was just refused.
                actions.push(Action::ToStore {
                    action: ActionStore::SaveStatus {
                        call_id: self.signup.calling(request_id, self.now),
                        item_id: item_id.clone(),
                        error_code: required.then(|| ITEM_LOGIN_REQUIRED.to_string()),
                    },
                });

                self.signup
                    .answer(request_id, ActionRouter::Accepted, self.now, actions);
            }

            WebhookFacts::Ignored {
                webhook_type,
                webhook_code,
            } => {
                tracing::debug!(webhook_type, webhook_code, "a webhook nothing acts on");

                self.signup
                    .answer(request_id, ActionRouter::Accepted, self.now, actions);
            }
        }
    }

    /// Stripe answered.
    ///
    /// Two calls in a row make one subscription, so the first answer decides
    /// whether the second is still wanted — which is why they are two events
    /// and not one helper.
    fn handle_event_stripe(&mut self, event: &EventSourceStripe, actions: &mut Vec<Action>) {
        match event {
            EventSourceStripe::CustomerCreated {
                call_id,
                user_id,
                customer_id,
            } => {
                let Some(request_id) = self.signup.answered(*call_id) else {
                    return;
                };

                let Some(Pending::Subscribing { user_id: waiting }) =
                    self.signup.resumed(request_id)
                else {
                    tracing::error!(
                        call_id,
                        "a customer answered a request nobody is waiting on"
                    );

                    return;
                };

                debug_assert_eq!(
                    waiting, *user_id,
                    "a customer answered another user's request"
                );

                self.signup.waits_on(
                    request_id,
                    Pending::SubscribingTo {
                        user_id: *user_id,
                        customer_id: customer_id.clone(),
                    },
                );

                let call_id = self.signup.calling(request_id, self.now);

                self.billing
                    .customer_ready(*user_id, customer_id, call_id, actions);
            }

            EventSourceStripe::SubscriptionCreated {
                call_id,
                user_id,
                subscription_id,
                status,
                client_secret,
            } => {
                let Some(request_id) = self.signup.answered(*call_id) else {
                    return;
                };

                let Some(Pending::SubscribingTo {
                    user_id: waiting,
                    customer_id,
                }) = self.signup.resumed(request_id)
                else {
                    tracing::error!(
                        call_id,
                        "a subscription answered a request nobody is waiting on"
                    );

                    return;
                };

                debug_assert_eq!(
                    waiting, *user_id,
                    "a subscription answered another user's request"
                );

                // Written whatever happens next, because the webhook that says
                // this was paid for names only the subscription: without the
                // row there is nothing to look the user up by.
                let write = self.signup.calling(request_id, self.now);

                self.billing.subscription_ready(
                    *user_id,
                    &customer_id,
                    subscription_id,
                    status,
                    write,
                    actions,
                );

                // No secret is a subscription with nothing to pay, which on a
                // freshly created one means Stripe answered in a shape this
                // build did not expect. Nothing to hand a card form, so the
                // caller is told it is unavailable rather than given a secret
                // that is not one.
                let Some(client_secret) = client_secret else {
                    tracing::error!(
                        subscription_id,
                        "stripe created a subscription with nothing to confirm against"
                    );

                    self.signup
                        .answer(request_id, ActionRouter::Unavailable, self.now, actions);

                    return;
                };

                self.signup.answer(
                    request_id,
                    ActionRouter::Subscribing {
                        client_secret: client_secret.clone(),
                    },
                    self.now,
                    actions,
                );
            }

            EventSourceStripe::StripeFailed {
                call_id,
                user_id,
                message,
            } => {
                let Some(request_id) = self.signup.answered(*call_id) else {
                    return;
                };

                // Dropped whichever step it was at: neither has anything to
                // resume from, and the caller can ask again.
                let _ = self.signup.resumed(request_id);

                tracing::error!(%user_id, message, "stripe would not start a subscription");

                self.signup
                    .answer(request_id, ActionRouter::Unavailable, self.now, actions);
            }
        }
    }

    fn handle_event_source(&mut self, event: &EventSourcePlaid, actions: &mut Vec<Action>) {
        match event {
            EventSourcePlaid::WebhookKeyLoaded { call_id, kid, key } => {
                // A miss that is not written down is a call to Plaid for
                // every POST naming an id nobody has.
                self.keys.insert(
                    kid.clone(),
                    CachedKey {
                        key: key.clone(),
                        at: self.now,
                    },
                );

                let Some(request_id) = self.signup.answered(*call_id) else {
                    return;
                };

                let Some(Pending::WebhookKey { jwt, body }) = self.signup.resumed(request_id)
                else {
                    tracing::error!(call_id, "a key answered a webhook nobody is waiting on");

                    return;
                };

                let Some(key) = key else {
                    tracing::warn!(kid, "a webhook named a key plaid does not have");

                    self.signup
                        .answer(request_id, ActionRouter::Unauthorized, self.now, actions);

                    return;
                };

                // Parsed again rather than kept: base64 twice is cheaper than
                // a variant holding a borrow across turns.
                let Ok(parsed) = PlaidWebhookJwt::parse(&jwt) else {
                    unreachable!("this token parsed on the turn it arrived")
                };

                self.verified(request_id, &parsed, key, &body, actions);
            }

            EventSourcePlaid::BalancesLoaded {
                call_id,
                item_id,
                connection,
            } => {
                if let Some(syncing) = self.syncing.get_mut(item_id) {
                    syncing.institution = Some(connection.name.clone());
                    syncing.accounts = connection.accounts.len().try_into().unwrap_or(u32::MAX);
                    syncing.stage = SyncStage::Transactions;

                    let user_id = syncing.user_id;

                    self.push_status(user_id, actions);
                }

                // Written down by the call that brought it, whatever becomes
                // of the rest of the sync. Nothing here holds a ledger to
                // assign over, the trap `docs/PLAID.md` describes.
                actions.push(Action::ToStore {
                    action: ActionStore::SaveAccounts {
                        call_id: self.signup.alongside(*call_id, self.now),
                        item_id: item_id.clone(),
                        institution_name: Some(connection.name.clone()),
                        accounts: connection.accounts.clone(),
                    },
                });

                // A sync nothing is recording is one that was given up on, so
                // there is nothing to ask Plaid next.
                let Some(cursor) = self.syncing.get(item_id).map(|s| s.cursor.clone()) else {
                    self.signup.landed(*call_id, self.now, actions);

                    return;
                };

                actions.push(Action::ToSourcePlaid {
                    action: ActionSourcePlaid::SyncTransactions {
                        call_id: self.signup.passing(*call_id, self.now),
                        item_id: item_id.clone(),
                        cursor,
                    },
                });
            }

            // The lists are a **diff** handed straight to the store.
            EventSourcePlaid::TransactionsLoaded {
                call_id,
                item_id,
                added,
                modified,
                removed,
                cursor,
                has_more,
            } => {
                // A page beyond the first spends a slot of its own. Plaid
                // meters `/transactions/sync` per Item per minute and a first
                // history is several pages, so a chain that asked nobody
                // would be the one way past the budget. Stopping costs
                // little: the write below saves the cursor to resume from.
                let another_page =
                    *has_more && self.may_call(PlaidEndpoint::TransactionsSync, Some(item_id));

                if *has_more && !another_page {
                    tracing::warn!(
                        item_id,
                        "out of budget part-way through a history; the next sync resumes",
                    );
                }

                if let Some(syncing) = self.syncing.get_mut(item_id) {
                    syncing.transactions = syncing
                        .transactions
                        .saturating_add(added.len().try_into().unwrap_or(u32::MAX));

                    syncing.cursor = Some(cursor.clone());

                    if !another_page {
                        syncing.stage = SyncStage::Streams;
                    }

                    let user_id = syncing.user_id;

                    self.push_status(user_id, actions);
                }

                actions.push(Action::ToStore {
                    action: ActionStore::SaveTransactions {
                        call_id: self.signup.alongside(*call_id, self.now),
                        item_id: item_id.clone(),
                        added: added.clone(),
                        modified: modified.clone(),
                        removed: removed.clone(),
                        cursor: cursor.clone(),
                    },
                });

                // A sync nothing is recording is one that was given up on.
                let Some(cursor) = self.syncing.get(item_id).map(|s| s.cursor.clone()) else {
                    self.signup.landed(*call_id, self.now, actions);

                    return;
                };

                let call_id = self.signup.passing(*call_id, self.now);
                let item_id = item_id.clone();

                let action = if another_page {
                    ActionSourcePlaid::SyncTransactions {
                        call_id,
                        item_id,
                        cursor,
                    }
                } else {
                    ActionSourcePlaid::FetchRecurring { call_id, item_id }
                };

                actions.push(Action::ToSourcePlaid { action });
            }

            EventSourcePlaid::RecurringLoaded {
                call_id,
                item_id,
                streams,
            } => {
                if let Some(syncing) = self.syncing.get_mut(item_id) {
                    syncing.streams = streams.len().try_into().unwrap_or(u32::MAX);
                }

                self.sync_finished(item_id, actions);

                // The wait passes from the last call to the last write, so
                // the route is answered once the rows are there.
                actions.push(Action::ToStore {
                    action: ActionStore::SaveStreams {
                        call_id: self.signup.passing(*call_id, self.now),
                        item_id: item_id.clone(),
                        streams: streams.clone(),
                    },
                });
            }

            EventSourcePlaid::LinkOpened {
                call_id,
                user_id,
                link_token,
                hosted_link_url,
            } => {
                let Some(request_id) = self.signup.answered(*call_id) else {
                    return;
                };

                let step = self.signup.resumed(request_id);

                debug_assert!(
                    matches!(step, Some(Pending::Opening)),
                    "the route asked for a session"
                );

                self.signup.link_opened(
                    request_id,
                    *user_id,
                    link_token,
                    hosted_link_url,
                    self.now,
                    actions,
                );
            }

            EventSourcePlaid::LinksPolled { call_id, polls } => {
                let Some(request_id) = self.signup.answered(*call_id) else {
                    return;
                };

                let step = self.signup.resumed(request_id);

                debug_assert!(
                    matches!(step, Some(Pending::Polling)),
                    "the route asked for a poll"
                );

                self.signup
                    .links_polled(request_id, polls, &mut self.limiter, self.now, actions);
            }

            // Named by the call rather than by the route it was made for: an
            // exchange outlives the request that asked for it, and what it
            // says has to be acted on either way.
            EventSourcePlaid::ItemLinked {
                call_id,
                link_token,
                item_id,
            } => {
                self.signup
                    .item_linked(*call_id, link_token, item_id, self.now, actions);
            }

            EventSourcePlaid::LinkAbandoned {
                call_id,
                link_token,
            } => {
                self.signup
                    .link_abandoned(*call_id, link_token, self.now, actions);
            }

            EventSourcePlaid::PlaidFailed {
                call_id,
                item_id,
                stage,
                fault,
            } => {
                // The recurring call is the one a sync can do without.
                // Transactions are what the app is for, and a missing list of
                // charges costs one screen rather than all of them.
                if *stage == SyncStage::Streams {
                    tracing::warn!(
                        item_id,
                        code = fault.as_str(),
                        "recurring unavailable, the sync stands without it",
                    );

                    self.sync_finished(item_id, actions);
                    self.signup.landed(*call_id, self.now, actions);

                    return;
                }

                self.sync_stopped(item_id, actions);

                if *fault == PlaidFault::LoginRequired {
                    actions.push(Action::ToProcess {
                        action: ActionProcess::ReportLoginRequired,
                    });

                    actions.push(Action::ToStore {
                        action: ActionStore::SaveStatus {
                            call_id: self.signup.passing(*call_id, self.now),
                            item_id: item_id.clone(),
                            error_code: Some(fault.as_str().to_string()),
                        },
                    });

                    return;
                }

                // Nothing to write, so the route is not left hanging.
                self.signup.landed(*call_id, self.now, actions);
            }
        }
    }

    fn handle_event_process(&mut self, event: &EventProcess, actions: &mut Vec<Action>) {
        match event {
            // Nothing is pulled into memory at boot: the store is read per
            // request, which is what makes a second server process possible.
            EventProcess::Started {
                collect,
                stripe_webhook_secret,
            } => {
                self.signup.started(*collect);
                self.billing.started(stripe_webhook_secret.clone());
            }

            EventProcess::Seeded { seed } => {
                // A seed on the way out is dropped: taking it would put a
                // stopping machine back in service.
                if self.shutting_down() {
                    return;
                }

                self.signup.seed(seed);
            }

            EventProcess::Tick { now } => {
                // Time only ever moves forward: a stale reading would make a
                // freshness window wrong in a way nothing else here catches.
                debug_assert!(
                    *now >= self.now,
                    "time went backwards: {now} after {}",
                    self.now,
                );

                self.now = *now;

                // Or the limiters' maps grow for the life of the process:
                // one is keyed by whoever calls us and the other by every bank
                // we have ever fetched.
                self.requests.sweep(self.now);
                self.limiter.sweep(self.now);
                self.sweep_webhook_keys();

                // A route waiting on Plaid to report a Link session first, so
                // that one let go of here is not then given up on as a call.

                self.signup.link_timed_out(self.now, actions);
                self.signup.given_up(self.now, actions);

                // Last, so a route let go of is decided before housekeeping.
                // Nothing new starts once we are on the way out.
                if !self.shutting_down() {
                    self.signup.sweep_sessions(self.now, actions);
                }
            }
        }
    }

    fn handle_event_store(
        &mut self,
        call_id: CallId,
        event: &EventStore,
        actions: &mut Vec<Action>,
    ) {
        match event {
            // The startup read. Both the entitlement each user has and the
            // index from a subscription to its user come from these rows, so
            // they are filled from one place and cannot disagree.
            EventStore::SubscriptionsLoaded { subscriptions } => {
                self.billing.loaded(subscriptions);

                for stored in subscriptions {
                    self.entitled.insert(
                        stored.user_id,
                        Entitlement {
                            provider: stored.provider,
                            status: stored.status.clone(),
                            good_until: stored.current_period_end,
                            cancel_at_period_end: stored.cancel_at_period_end,
                        },
                    );
                }

                tracing::info!(count = subscriptions.len(), "subscriptions loaded");
            }

            // The address a subscription's receipt goes to, which is the last
            // thing needed before Stripe can be asked for anything.
            EventStore::UserLoaded { email } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::Loading { user_id, .. }) = self.signup.resumed(request_id) else {
                    tracing::error!(call_id, "the store answered a subscribe nobody asked for");

                    return;
                };

                // A live session naming a user the store does not have should
                // not happen: sessions are ON DELETE CASCADE from users.
                let Some(email) = email else {
                    tracing::error!(%user_id, "a session names a user the store does not have");

                    self.signup
                        .answer(request_id, ActionRouter::Unauthorized, self.now, actions);

                    return;
                };

                self.signup
                    .waits_on(request_id, Pending::Subscribing { user_id });

                let call_id = self.signup.calling(request_id, self.now);

                self.billing
                    .subscribe_requested(user_id, email, call_id, actions);
            }

            EventStore::CodeLoaded { stored } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::Submitted { email, code }) = self.signup.resumed(request_id)
                else {
                    tracing::error!(call_id, "the store answered a sign-in nobody submitted");

                    return;
                };

                self.signup.code_loaded(
                    request_id,
                    &email,
                    &code,
                    stored.as_ref(),
                    self.now,
                    actions,
                );
            }

            EventStore::ItemLoaded { item } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::WebhookItem { item_id }) = self.signup.resumed(request_id) else {
                    tracing::error!(call_id, "the store answered a bank nobody asked about");

                    return;
                };

                self.handle_webhook(
                    request_id,
                    &WebhookFacts::SyncUpdatesAvailable {
                        item_id,
                        item: item.clone(),
                    },
                    actions,
                );
            }

            EventStore::ItemSaved { item } => {
                let Some(item) = item else {
                    self.signup.item_unattached(call_id, self.now, actions);

                    return;
                };

                // Now rather than on the next `/snapshot`: the first sync of a
                // new bank is the long one, and a screen watching a sync needs
                // one running. Nobody waits on it — `/link/finish` would stay
                // open for the whole of it.
                let fetching = self.signup.call();

                // A bank attached seconds ago has no cursor to resume from.
                self.fetch(fetching, item.item_id.clone(), item.user_id, None, actions);

                self.signup.item_attached(call_id, self.now, actions);
            }

            EventStore::ItemsLoaded { items } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::Loading { user_id, event }) = self.signup.resumed(request_id)
                else {
                    tracing::error!(call_id, "the store answered banks nobody asked about");

                    return;
                };

                match event {
                    Authenticated::SnapshotRequested => {
                        self.handle_snapshot(request_id, user_id, items, actions)
                    }

                    _ => unreachable!("only a snapshot asks for banks"),
                }
            }

            EventStore::PendingLinksLoaded { link_tokens } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::Loading { .. }) = self.signup.resumed(request_id) else {
                    tracing::error!(call_id, "the store answered sessions nobody asked about");

                    return;
                };

                self.signup.pending_links_loaded(
                    request_id,
                    link_tokens,
                    &mut self.limiter,
                    self.now,
                    actions,
                );
            }

            EventStore::SnapshotLoaded { snapshot } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let step = self.signup.resumed(request_id);

                debug_assert!(
                    matches!(step, Some(Pending::Reading)),
                    "the route asked for its ledger"
                );

                self.signup.answer(
                    request_id,
                    ActionRouter::Serve {
                        snapshot: snapshot.clone(),
                    },
                    self.now,
                    actions,
                );
            }

            EventStore::SessionResolved { user_id } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::Authenticating { ip, event }) = self.signup.resumed(request_id)
                else {
                    tracing::error!(call_id, "the store answered a session nobody asked about");

                    return;
                };

                self.handle_authenticated(request_id, ip, &event, *user_id, actions);
            }

            // There is no answer to fall back on: a sign-in refused because
            // the store was down is indistinguishable from a wrong code.
            EventStore::ReadError { message } => {
                tracing::error!(message, "could not read the store");

                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                // Whatever the route was doing, it cannot do it without the
                // read.
                self.signup.resumed(request_id);

                self.signup
                    .answer(request_id, ActionRouter::Unavailable, self.now, actions);
            }

            EventStore::WriteSuccess => self.signup.landed(call_id, self.now, actions),

            // Ends the wait: a route held for a failed write is held for
            // ever. The one exception is the write that attaches a newly
            // linked bank, which is asked for again — what it is writing down
            // is an Item slot already spent.
            EventStore::WriteError { message } => {
                tracing::error!(message, "could not write the store");

                if self.signup.attach_refused(call_id, self.now, actions) {
                    return;
                }

                self.signup.landed(call_id, self.now, actions);
            }
        }
    }

    fn handle_event_terminal(&mut self, event: &EventTerminal, actions: &mut Vec<Action>) {
        match event {
            EventTerminal::SigInt | EventTerminal::SigTerm => {
                // A second ^C while the first drains queues no second one.
                if self.shutting_down() {
                    return;
                }

                tracing::info!("shutting down");

                self.state = Lifecycle::Stopping;

                self.signup.stopping();

                actions.push(Action::ToProcess {
                    action: ActionProcess::Shutdown,
                });
            }
        }
    }

    /// Start a sync of one Item, and remember that we have — both in the same
    /// turn, or a screen watching it reads the interval where nothing says so
    /// as "finished".
    ///
    /// Only the first of the calls goes out here. What follows it is decided
    /// when each answer lands, which is what puts the order, the paging and
    /// the budget in one place a test can reach.
    fn fetch(
        &mut self,
        call_id: CallId,
        item_id: String,
        user_id: Uuid,
        cursor: Option<String>,
        actions: &mut Vec<Action>,
    ) {
        self.syncing
            .insert(item_id.clone(), Syncing::started(user_id, cursor));

        // In the same turn it starts: a screen that opened its stream while
        // nothing was running is told that something now is.
        self.push_status(user_id, actions);

        actions.push(Action::ToSourcePlaid {
            action: ActionSourcePlaid::FetchBalances {
                call_id,
                item_id,
                now: self.now,
            },
        });
    }

    /// Whether Plaid's budget for this endpoint has room, and spend it if it
    /// has.
    ///
    /// `item_id` is the bank the request is about, where there is one: Plaid
    /// meters those per Item, so two banks do not share a budget. The requests
    /// that name no bank are counted for the account.
    fn may_call(&mut self, endpoint: PlaidEndpoint, item_id: Option<&str>) -> bool {
        let now = self.now;

        let ctx = match item_id {
            Some(item_id) => PlaidRateCtx::new(endpoint, item_id),
            None => PlaidRateCtx::account(endpoint),
        };

        self.limiter.can_send(now, &ctx)
    }

    /// A sync is over and what it brought stands. Kept rather than dropped:
    /// this is what a screen watching one reads to say it is done, and the
    /// totals it draws under that.
    ///
    /// A sync with no record of having started is one that was given up on.
    /// What its calls brought is written down all the same by whatever else
    /// the event asked for; there is just nobody it is known to belong to.
    fn sync_finished(&mut self, item_id: &str, actions: &mut Vec<Action>) {
        let Some(syncing) = self.syncing.get_mut(item_id) else {
            return;
        };

        syncing.finished = true;
        syncing.stage = SyncStage::Streams;

        let user_id = syncing.user_id;

        self.push_status(user_id, actions);
    }

    /// Stop reporting a sync. Dropped rather than marked done: a screen
    /// watching one that died would otherwise sit on its last stage.
    fn sync_stopped(&mut self, item_id: &str, actions: &mut Vec<Action>) {
        let Some(syncing) = self.syncing.remove(item_id) else {
            return;
        };

        self.push_status(syncing.user_id, actions);
    }

    /// Whether to go back to Plaid about this bank.
    ///
    /// `force` is the machine's alone, and only a webhook sets it: reachable
    /// from the wire, a gesture anybody can repeat becomes a way to spend this
    /// account's Plaid budget.
    fn should_fetch(&mut self, item: &ItemFacts, force: bool) -> bool {
        // Nothing new starts once we are on the way out.
        if self.shutting_down() {
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

        // A sync is three calls, and the strictest budget decides. Asked last
        // so a request refused for freshness does not spend a slot. A history
        // long enough to page asks again for each page after the first.
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

    /// How much the machine is holding. See [`CensusServer`].
    ///
    /// The one thing about this machine no action reports, and so the only way
    /// a test can say that a flow gave back everything it took.
    pub fn census(&self) -> CensusServer {
        CensusServer {
            waiting: self.signup.waiting(),
            requests: self.signup.requests(),
            syncing: self.syncing.len(),
            watchers: self.watchers.len(),
            sessions: self.signup.sessions(),
            exchanging: self.signup.exchanging(),
            attaching: self.signup.attaching(),
            keys: self.keys.len(),
            request_windows: self.requests.kept(),
            plaid_windows: self.limiter.kept(),
        }
    }

    /// Whether the machine is on its way out, drained or not.
    pub fn shutting_down(&self) -> bool {
        matches!(self.state, Lifecycle::Stopping | Lifecycle::Stopped)
    }

    /// Whether there is nothing left to do. The runtime's loop runs until this.
    pub fn stopped(&self) -> bool {
        self.state == Lifecycle::Stopped
    }

    /// Come to a stop, once there is nothing left that could still answer. At
    /// the end of every batch rather than on the tick alone, so a signal on an
    /// idle server stops it in the turn it arrives in.
    fn settle(&mut self) {
        if self.state != Lifecycle::Stopping {
            return;
        }

        if !self.signup.quiet() {
            return;
        }

        tracing::info!("stopped");

        self.state = Lifecycle::Stopped;
    }
}
