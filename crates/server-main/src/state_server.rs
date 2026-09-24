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
use std::time::Duration;

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
use src_postgres::BarSource;
use src_stripe::StripeStatus;
use uuid::Uuid;

use crate::Action;
use crate::ActionProcess;
use crate::ActionRouter;
use crate::ActionSocket;
use crate::ActionSourcePlaid;
use crate::ActionSourceStripe;
use crate::ActionStore;
use crate::AppleFacts;
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
use crate::EventSourceYahoo;
use crate::EventStore;
use crate::EventTerminal;
use crate::ItemFacts;
use crate::Lifecycle;
use crate::Owed;
use crate::Pending;
use crate::RequestId;
use crate::StateBilling;
use crate::StateMarkets;
use crate::StateSignup;
use crate::StripePayer;
use crate::Syncing;
use crate::WebhookFacts;
use crate::entitlement;
use crate::login_code;
use crate::rate_limit::ServerRateCtx;
use crate::rate_limit::ServerRateLimiter;
use crate::state_markets::BARS_ASKED_AT_MOST;
use crate::state_markets::bars_asked;

/// How long a sync is good for before Plaid is asked again.
///
/// Every call the window gates is free at the margin — the two transaction
/// calls ride the per-Item subscription and `/accounts/get` answers from
/// Plaid's cache — so this protects rate limits rather than a bill, and
/// thirty seconds is short enough that a pull-to-refresh still feels live.
pub const FRESH_FOR_SECS: u64 = 30;

/// How long after each `PRODUCT_NOT_READY` the transactions are asked for
/// again, and — by running out — when a sync stops being asked for at all.
///
/// A fresh Item generates its history asynchronously, usually within
/// seconds, so the waits start short and grow. The call is covered by the
/// per-Item subscription, so the schedule is a rate-limit courtesy rather
/// than a cost.
pub const NOT_READY_RETRY_SECS: [u64; 4] = [5, 15, 30, 60];

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

/// The call the startup read of who has paid goes out on. Zero, which the
/// counter never hands out: it is the one call made before any route exists,
/// and nothing waits on it but the machine itself.
pub const STARTUP_READ: CallId = 0;

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

    /// Fetching market data: what to ask the sources, in what order, and how
    /// fast. Its own process too; the bars themselves live in the store.
    markets: StateMarkets,

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
    syncing: BTreeMap<String, Syncing>,

    /// Who is being written to, and on whose behalf. Keyed by the connection
    /// so that letting one go is a removal, and capped per user by
    /// [`WATCHERS_AT_MOST`].
    watchers: BTreeMap<ConnId, Uuid>,

    /// When the current batch is being handled.
    now: Timestamp,

    /// What is known about each webhook signing key, by the `kid` a token
    /// named. Swept on the tick: these names come from strangers.
    keys: BTreeMap<String, CachedKey>,
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
            Event::FromSourcePlaid { call_id, event } => {
                self.handle_event_source(*call_id, event, actions)
            }
            Event::FromSourceStripe { call_id, event } => {
                self.handle_event_stripe(*call_id, event, actions)
            }
            Event::FromSourceYahoo { call_id, event } => {
                self.handle_event_yahoo(*call_id, event, actions)
            }
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
        // Every signed-in route is decided from who has paid, and so are both
        // of the webhooks that say so. Until the startup read is in, that
        // table is empty rather than known; unavailable is the true answer,
        // and it is the one Stripe and Apple send again.
        if self.starting()
            && (event.token().is_some()
                || matches!(
                    event,
                    EventRouter::StripeWebhookReceived { .. }
                        | EventRouter::AppleNotificationReceived { .. }
                ))
        {
            tracing::info!("a request arrived before the server knew who has paid");

            self.signup
                .answer(request_id, ActionRouter::Unavailable, self.now, actions);

            return;
        }

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
                call_id: self.signup.calling(request_id, self.now),
                action: ActionStore::LoadSession {
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
            EventRouter::LinkRequested { .. } | EventRouter::RepairRequested { .. } => {
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

            // Apple's half of the same thing, and believed the same way. What
            // differs is that there is no secret to check it against: the
            // payload carries the certificate chain that signed it, and the
            // root it has to reach is the one committed in `src-apple`.
            EventRouter::AppleNotificationReceived { signed } => {
                self.believe_apple(request_id, signed, actions)
            }

            EventRouter::SnapshotRequested { .. }
            | EventRouter::SubscribeRequested { .. }
            | EventRouter::CheckoutRequested { .. }
            | EventRouter::BillingStatusRequested { .. }
            | EventRouter::AppleTransactionReceived { .. }
            | EventRouter::DeleteCodeRequested { .. }
            | EventRouter::DeleteRequested { .. }
            | EventRouter::BarsRequested { .. } => {
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

        let event_at = u64::try_from(event.created)
            .map(Timestamp::from_secs)
            .unwrap_or(self.now);

        let entitlement = StateBilling::entitlement_of(subscription, event_at, self.now);

        // Held only for a user this process knows about. A subscription it
        // does not know but whose customer it does is one born on Stripe's
        // own Checkout page — this webhook is the first news of it, so the
        // row the next lookup needs is written now, the way Apple's first
        // purchase is. Neither is a subscription created against another
        // database — a test key pointed at a live server, or a customer from
        // before this table existed — which is what the last arm is.
        match self.billing.user_for_subscription(&subscription.id) {
            Some(user_id) => {
                self.entitle(user_id, entitlement);
            }
            None => match self.billing.user_for_customer(&subscription.customer) {
                Some(user_id) => {
                    self.entitle(user_id, entitlement);

                    let write = self.signup.calling(request_id, self.now);

                    self.billing.subscription_ready(
                        user_id,
                        &subscription.customer,
                        &subscription.id,
                        &subscription.status(),
                        write,
                        actions,
                    );
                }
                None => {
                    tracing::warn!(
                        subscription = subscription.id,
                        "a webhook named a subscription this process does not know"
                    );
                }
            },
        }

        // Sent whatever the lookup said. The row may exist in the store even
        // where this process has not got it in hand, and the query is keyed by
        // the subscription rather than the user.
        self.write_subscription(
            request_id,
            subscription,
            event.id.clone(),
            event_at,
            actions,
        );
    }

    /// Whether a notification is really Apple's, and what to do about it.
    ///
    /// **Answered in the same turn the chain checks out**, before the write
    /// lands, for the reason [`believe_stripe`](Self::believe_stripe) is:
    /// Apple retries a delivery it is not told about, for days, so waiting for
    /// the store would turn one renewal into a pile of them. The ordering
    /// guard on the write is what makes that safe.
    fn believe_apple(&mut self, request_id: RequestId, signed: &str, actions: &mut Vec<Action>) {
        let Some(facts) = self.billing.believes_apple_notification(signed, self.now) else {
            self.signup
                .answer(request_id, ActionRouter::Unauthorized, self.now, actions);

            return;
        };

        self.signup
            .answer(request_id, ActionRouter::Accepted, self.now, actions);

        // Acted on only for a payer this process knows about. One it does not
        // is a purchase made against another database — a sandbox tester on a
        // server that has been reset, or an account since deleted.
        let Some(user_id) = self.billing.user_for_apple_payer(&facts.customer_id) else {
            tracing::warn!(
                customer = facts.customer_id,
                "a notification named a payer this process does not know",
            );

            return;
        };

        self.apple_landed(request_id, user_id, &facts, actions);
    }

    /// Hold what Apple said, and write it.
    ///
    /// Both doors end here — the app handing over a purchase and Apple
    /// reporting a renewal — because what is done about a transaction does not
    /// depend on which of them carried it.
    fn apple_landed(
        &mut self,
        request_id: RequestId,
        user_id: Uuid,
        facts: &AppleFacts,
        actions: &mut Vec<Action>,
    ) {
        self.entitle(user_id, facts.entitlement());

        // So the next notification, which names the payer and not the user,
        // finds whose row to move.
        self.billing.apple_payer(&facts.customer_id, user_id);

        actions.push(Action::ToStore {
            call_id: self.signup.calling(request_id, self.now),
            action: ActionStore::ApplyAppleEvent {
                user_id,
                customer_id: facts.customer_id.clone(),
                status: facts.status.as_str().to_string(),
                current_period_end: facts.good_until,
                event_id: facts.event_id.clone(),
                event_at: facts.event_at,
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

        // Known from here on, so whatever this route ends up serving can be
        // written down against who it was served to.
        self.signup.identify(request_id, user_id, ip.clone());

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

            // Why, and not only that: one status code is four screens, and
            // which one is a fact only this side holds.
            let unpaid = entitlement::unpaid(self.entitled.get(&user_id));

            self.signup.answer(
                request_id,
                ActionRouter::PaymentRequired { unpaid },
                self.now,
                actions,
            );

            return;
        }

        match event {
            Authenticated::LinkRequested { origin } => self.signup.link_requested(
                request_id,
                user_id,
                origin.as_deref(),
                &mut self.limiter,
                self.now,
                actions,
            ),

            // Whose Item it is has to be settled before Plaid is asked for
            // anything, so the caller's banks are read first.
            Authenticated::RepairRequested { .. } => {
                self.signup.waits_on(
                    request_id,
                    Pending::Loading {
                        user_id,
                        event: event.clone(),
                    },
                );

                actions.push(Action::ToStore {
                    call_id: self.signup.calling(request_id, self.now),
                    action: ActionStore::LoadItems { user_id },
                });
            }

            Authenticated::LinkClosed => {
                self.signup
                    .link_closing(request_id, user_id, self.now, actions);
            }

            // Nothing to read: what is running is the machine's own state, and
            // it is the only thing that knows.
            Authenticated::SyncWatchRequested { conn_id } => {
                self.handle_sync_watch(request_id, *conn_id, user_id, actions)
            }

            // The plan for a bars request is decided by what is covered, so
            // the coverage is read first.
            Authenticated::BarsRequested {
                symbol,
                interval,
                start,
                end,
            } => {
                // Refused before anything is read: a backwards window has no
                // answer, and a window naming more bars than anything draws
                // is a read nobody should pay for.
                if *start >= *end || bars_asked(*interval, &(*start..*end)) > BARS_ASKED_AT_MOST {
                    self.signup
                        .answer(request_id, ActionRouter::BadAsk, self.now, actions);

                    return;
                }

                self.signup.waits_on(
                    request_id,
                    Pending::PlanningBars {
                        symbol: symbol.clone(),
                        interval: *interval,
                        start: *start,
                        end: *end,
                    },
                );

                actions.push(Action::ToStore {
                    call_id: self.signup.calling(request_id, self.now),
                    action: ActionStore::LoadBarCoverage {
                        source: BarSource::Yahoo,
                        symbol: symbol.clone(),
                        interval: *interval,
                    },
                });
            }

            // The rows this route decides from, scoped to the caller: two
            // users share a process and share no cursors.
            Authenticated::SnapshotRequested => {
                self.signup.waits_on(
                    request_id,
                    Pending::Loading {
                        user_id,
                        event: event.clone(),
                    },
                );

                actions.push(Action::ToStore {
                    call_id: self.signup.calling(request_id, self.now),
                    action: ActionStore::LoadItems { user_id },
                });
            }

            Authenticated::SubscribeRequested => {
                // Refused before Stripe is asked for anything: a secret with
                // no key to open a card form with is a subscription nobody can
                // pay, left behind in Stripe.
                if self.billing.publishable_key().is_none() {
                    tracing::error!("asked to subscribe with no publishable key to hand out");

                    self.signup
                        .answer(request_id, ActionRouter::Unavailable, self.now, actions);

                    return;
                }

                // A press while the last one is still being answered. Stripe
                // is asked nothing: the one in flight is the subscription
                // this person gets, and it is theirs to pick up once it
                // exists.
                if self.signup.subscribing(user_id) {
                    tracing::info!(%user_id, "asked to subscribe while a subscribe is in flight");

                    self.signup
                        .answer(request_id, ActionRouter::Subscribed, self.now, actions);

                    return;
                }

                // What this press means depends on what Stripe has already
                // been asked for, and asking that first is what stops a
                // second subscription being made for somebody who is halfway
                // through one. Every arm here answers the same route; they
                // differ in how much of the conversation is already done.
                match self.billing.payer(user_id).cloned() {
                    // A subscription already exists. Read it back rather than
                    // making another: one somebody walked away from is still
                    // waiting for a card, and its own invoice is what they
                    // should be handed when they come back.
                    Some(StripePayer {
                        subscription_id: Some(subscription_id),
                        ..
                    }) => {
                        self.signup
                            .waits_on(request_id, Pending::Subscribing { user_id });

                        actions.push(Action::ToSourceStripe {
                            call_id: self.signup.calling(request_id, self.now),
                            action: ActionSourceStripe::LoadSubscription {
                                user_id,
                                subscription_id,
                            },
                        });
                    }

                    // A customer with nothing against it: Stripe took the
                    // first call and refused the second. The address has
                    // already been spent, so this picks up where that left
                    // off.
                    Some(StripePayer { customer_id, .. }) => {
                        self.signup.waits_on(
                            request_id,
                            Pending::SubscribingTo {
                                user_id,
                                customer_id: customer_id.clone(),
                            },
                        );

                        actions.push(Action::ToSourceStripe {
                            call_id: self.signup.calling(request_id, self.now),
                            action: ActionSourceStripe::CreateSubscription {
                                user_id,
                                customer_id,
                            },
                        });
                    }

                    // Nobody Stripe has heard of. The address is what Stripe
                    // emails a receipt to and the machine does not hold one,
                    // so the store is asked and the conversation with Stripe
                    // starts a turn later.
                    None => {
                        self.signup.waits_on(
                            request_id,
                            Pending::Loading {
                                user_id,
                                event: event.clone(),
                            },
                        );

                        actions.push(Action::ToStore {
                            call_id: self.signup.calling(request_id, self.now),
                            action: ActionStore::LoadUser { user_id },
                        });
                    }
                }
            }

            Authenticated::CheckoutRequested { origin } => {
                // Refused before Stripe is asked for anything: a Checkout
                // page with nowhere to send its window back is one whose
                // ending the app never hears about.
                let Some((success_url, cancel_url)) = self.signup.checkout_urls(origin.as_deref())
                else {
                    self.signup
                        .answer(request_id, ActionRouter::Rejected, self.now, actions);

                    return;
                };

                // A press while the last one is still being answered. Stripe
                // is asked nothing: the page in flight is the one this
                // person gets.
                if self.signup.subscribing(user_id) {
                    tracing::info!(%user_id, "asked for a checkout while one is in flight");

                    self.signup
                        .answer(request_id, ActionRouter::Subscribed, self.now, actions);

                    return;
                }

                // What this press means depends on what Stripe has already
                // been asked for. Every arm here answers the same route;
                // they differ in how much of the conversation is already
                // done.
                match self.billing.payer(user_id).cloned() {
                    // A subscription already exists. Read it back rather
                    // than selling another: if it is worth something, there
                    // is nothing to buy, and reading it is what repairs a
                    // webhook that never arrived.
                    Some(StripePayer {
                        subscription_id: Some(subscription_id),
                        ..
                    }) => {
                        self.signup.waits_on(
                            request_id,
                            Pending::CheckingOut {
                                user_id,
                                success_url,
                                cancel_url,
                            },
                        );

                        actions.push(Action::ToSourceStripe {
                            call_id: self.signup.calling(request_id, self.now),
                            action: ActionSourceStripe::LoadSubscription {
                                user_id,
                                subscription_id,
                            },
                        });
                    }

                    // A customer with nothing against it, so the page opens
                    // straight away.
                    Some(StripePayer { customer_id, .. }) => {
                        self.signup
                            .waits_on(request_id, Pending::OpeningCheckout { user_id });

                        actions.push(Action::ToSourceStripe {
                            call_id: self.signup.calling(request_id, self.now),
                            action: ActionSourceStripe::OpenCheckout {
                                user_id,
                                customer_id,
                                success_url,
                                cancel_url,
                            },
                        });
                    }

                    // Nobody Stripe has heard of. The address is what Stripe
                    // emails a receipt to and the machine does not hold one,
                    // so the store is asked and the conversation with Stripe
                    // starts a turn later.
                    None => {
                        self.signup.waits_on(
                            request_id,
                            Pending::Loading {
                                user_id,
                                event: event.clone(),
                            },
                        );

                        actions.push(Action::ToStore {
                            call_id: self.signup.calling(request_id, self.now),
                            action: ActionStore::LoadUser { user_id },
                        });
                    }
                }
            }

            // The code goes to the address the account is written to, and the
            // store is the only thing that holds one — the same first call a
            // subscribe with no customer makes.
            Authenticated::DeleteCodeRequested => {
                self.signup.waits_on(
                    request_id,
                    Pending::Loading {
                        user_id,
                        event: event.clone(),
                    },
                );

                actions.push(Action::ToStore {
                    call_id: self.signup.calling(request_id, self.now),
                    action: ActionStore::LoadUser { user_id },
                });
            }

            // Cancelling and deleting are one action, because a paused
            // account is one we keep paying Plaid for: an Item bills for as
            // long as it exists, so there is no state where somebody stops
            // paying us and we stop paying for them. Out is out. What stands
            // between the token and the rows is the emailed code, so the one
            // the store holds is read first.
            Authenticated::DeleteRequested { code } => {
                self.signup.waits_on(
                    request_id,
                    Pending::DeleteSubmitted {
                        user_id,
                        code: code.clone(),
                    },
                );

                actions.push(Action::ToStore {
                    call_id: self.signup.calling(request_id, self.now),
                    action: ActionStore::LoadDeleteCode { user_id },
                });
            }

            // The App Store took the money before we heard about it, so this
            // is not a request to buy. It is the only record that a purchase
            // happened, and the session is the one thing tying it to an
            // account — which is why it is the one billing route that is
            // authenticated and still open to somebody who has not paid.
            Authenticated::AppleTransactionReceived { signed } => {
                let Some(facts) = self.billing.believes_apple(signed, self.now) else {
                    self.signup
                        .answer(request_id, ActionRouter::Unauthorized, self.now, actions);

                    return;
                };

                // One payer, one account — the rule the `subscriptions` unique
                // index keeps, decided here so the answer says which it is
                // rather than arriving as a failed write. Somebody signed into
                // the wrong account can do something about this; a 500 tells
                // them nothing.
                match self.billing.user_for_apple_payer(&facts.customer_id) {
                    Some(owner) if owner != user_id => {
                        tracing::warn!(
                            customer = facts.customer_id,
                            "a purchase already paying for another account",
                        );

                        self.signup
                            .answer(request_id, ActionRouter::Claimed, self.now, actions);

                        return;
                    }
                    _ => {}
                }

                self.signup
                    .answer(request_id, ActionRouter::Accepted, self.now, actions);

                self.apple_landed(request_id, user_id, &facts, actions);
            }

            Authenticated::BillingStatusRequested => {
                let entitlement = self.entitled.get(&user_id);
                let entitled = self.allows(user_id);

                self.signup.answer(
                    request_id,
                    ActionRouter::BillingStatus {
                        entitled,

                        // Why, in our words, and only when there is a why.
                        // The provider's own word does not leave the server on
                        // this route either — it is the one a client reads
                        // most, so a leak here would be the whole of the leak.
                        unpaid: (!entitled).then(|| entitlement::unpaid(entitlement)),

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

    /// Cancel the subscription, stop the banks, and delete everything.
    ///
    /// Four steps, in an order chosen by what is worst if the process dies
    /// half way:
    ///
    /// 1. **Stripe first.** A card still being charged for an app that is gone
    ///    is the failure a person notices and the one that is hardest to undo
    ///    trust over. Everything else can be finished by hand from a log line.
    /// 2. **Read the tokens**, before anything is deleted: `items` goes with
    ///    the user, and a token nobody holds is a bank that bills forever.
    /// 3. **Remove the banks** at Plaid, which is the only way those bills
    ///    stop.
    /// 4. **Delete the rows**, once no removal is still out.
    ///
    /// A user with no subscription — never paid, or already cancelled — skips
    /// straight to step 2. Deleting an account is not something to refuse
    /// because there is nothing to cancel.
    fn delete_requested(
        &mut self,
        request_id: RequestId,
        user_id: Uuid,
        actions: &mut Vec<Action>,
    ) {
        // Asked of billing rather than of `entitled`: the question is whether
        // Stripe has something to cancel, not whether the app is open. A
        // subscription whose period has run out is still one Stripe would
        // renew, and skipping it because access had lapsed is how somebody
        // keeps being charged for an account that is gone.
        let Some(subscription_id) = self
            .billing
            .subscription_for_user(user_id)
            .map(str::to_string)
        else {
            self.read_tokens_to_delete(request_id, user_id, actions);

            return;
        };

        self.signup
            .waits_on(request_id, Pending::Deleting { user_id });

        actions.push(Action::ToSourceStripe {
            call_id: self.signup.calling(request_id, self.now),
            action: ActionSourceStripe::CancelSubscription {
                user_id,
                subscription_id,
                now: self.now,
            },
        });
    }

    /// Step 2: the tokens the bank removals need.
    fn read_tokens_to_delete(
        &mut self,
        request_id: RequestId,
        user_id: Uuid,
        actions: &mut Vec<Action>,
    ) {
        self.signup
            .waits_on(request_id, Pending::DeletingReadingTokens { user_id });

        actions.push(Action::ToStore {
            call_id: self.signup.calling(request_id, self.now),
            action: ActionStore::LoadAccessTokens { user_id },
        });
    }

    /// Step 3: stop every bank, or go straight to the rows if there are none.
    fn remove_banks_to_delete(
        &mut self,
        request_id: RequestId,
        user_id: Uuid,
        tokens: &[(String, String)],
        actions: &mut Vec<Action>,
    ) {
        if tokens.is_empty() {
            self.delete_the_rows(request_id, user_id, actions);

            return;
        }

        self.signup.waits_on(
            request_id,
            Pending::DeletingBanks {
                user_id,
                left: tokens.len() as u32,
            },
        );

        for (item_id, access_token) in tokens {
            actions.push(Action::ToSourcePlaid {
                call_id: self.signup.calling(request_id, self.now),
                action: ActionSourcePlaid::RemoveItem {
                    item_id: item_id.clone(),
                    access_token: access_token.clone(),
                },
            });
        }
    }

    /// Step 4: the rows, and the answer.
    ///
    /// Entitlement goes here rather than when the subscription was cancelled:
    /// until this lands the account still exists, and a half-deleted account
    /// that has been locked out is worse than one that still works.
    fn delete_the_rows(&mut self, request_id: RequestId, user_id: Uuid, actions: &mut Vec<Action>) {
        self.entitled.remove(&user_id);
        self.billing.forget(user_id);

        actions.push(Action::ToStore {
            call_id: self.signup.calling(request_id, self.now),
            action: ActionStore::DeleteUser { user_id },
        });

        // Owed, so it goes out once the delete has actually landed: a client
        // told its account is gone while the rows are still there would be
        // told a thing that is not yet true.
        self.signup
            .answer(request_id, ActionRouter::Deleted, self.now, actions);
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
            call_id: self.signup.calling(request_id, self.now),
            action: ActionSourcePlaid::FetchWebhookKey {
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
            // Not acted on until the store says whether the session repairs an
            // Item: a repair's finish must not be exchanged, and the exchange
            // is the call nothing can undo.
            PlaidWebhookKind::SessionFinished {
                link_token,
                public_tokens,
                exited,
            } => {
                self.signup.waits_on(
                    request_id,
                    Pending::WebhookLink {
                        link_token: link_token.clone(),
                        public_tokens,
                        exited,
                    },
                );

                actions.push(Action::ToStore {
                    call_id: self.signup.calling(request_id, self.now),
                    action: ActionStore::LoadLinkRepair { link_token },
                });

                return;
            }

            // The one kind that needs a read: it names a bank, not a person.
            PlaidWebhookKind::SyncUpdatesAvailable { item_id } => {
                self.signup.waits_on(
                    request_id,
                    Pending::WebhookItem {
                        item_id: item_id.clone(),
                    },
                );

                actions.push(Action::ToStore {
                    call_id: self.signup.calling(request_id, self.now),
                    action: ActionStore::LoadItem { item_id },
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
                    call_id: self.signup.calling(request_id, self.now),
                    action: ActionStore::SaveStatus {
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
    fn handle_event_stripe(
        &mut self,
        call_id: CallId,
        event: &EventSourceStripe,
        actions: &mut Vec<Action>,
    ) {
        match event {
            EventSourceStripe::CustomerCreated {
                user_id,
                customer_id,
            } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                match self.signup.resumed(request_id) {
                    Some(Pending::Subscribing { user_id: waiting }) => {
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

                    Some(Pending::CheckingOut {
                        user_id: waiting,
                        success_url,
                        cancel_url,
                    }) => {
                        debug_assert_eq!(
                            waiting, *user_id,
                            "a customer answered another user's request"
                        );

                        // Written down before the page opens, because the
                        // subscription the page sells arrives by webhook and
                        // the webhook names only the customer. A restart in
                        // between must still find whose customer this is.
                        self.billing.customer_created(customer_id, *user_id);

                        actions.push(Action::ToStore {
                            call_id: self.signup.calling(request_id, self.now),
                            action: ActionStore::SaveCustomer {
                                user_id: *user_id,
                                provider: src_postgres::BillingProvider::Stripe,
                                customer_id: customer_id.clone(),
                                subscription_id: None,
                                status: "none".to_string(),
                            },
                        });

                        self.signup
                            .waits_on(request_id, Pending::OpeningCheckout { user_id: *user_id });

                        actions.push(Action::ToSourceStripe {
                            call_id: self.signup.calling(request_id, self.now),
                            action: ActionSourceStripe::OpenCheckout {
                                user_id: *user_id,
                                customer_id: customer_id.clone(),
                                success_url,
                                cancel_url,
                            },
                        });
                    }

                    _ => {
                        tracing::error!("a customer answered a request nobody is waiting on");
                    }
                }
            }

            EventSourceStripe::SubscriptionCreated {
                user_id,
                subscription_id,
                status,
                client_secret,
            } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::SubscribingTo {
                    user_id: waiting,
                    customer_id,
                }) = self.signup.resumed(request_id)
                else {
                    tracing::error!("a subscription answered a request nobody is waiting on");

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

                self.hand_over_the_card_form(
                    request_id,
                    subscription_id,
                    client_secret.as_deref(),
                    actions,
                );
            }

            EventSourceStripe::CheckoutOpened { user_id, url } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::OpeningCheckout { user_id: waiting }) =
                    self.signup.resumed(request_id)
                else {
                    tracing::error!("a checkout page answered a request nobody is waiting on");

                    return;
                };

                debug_assert_eq!(
                    waiting, *user_id,
                    "a checkout page answered another user's request"
                );

                self.signup.answer(
                    request_id,
                    ActionRouter::CheckoutOpened { url: url.clone() },
                    self.now,
                    actions,
                );
            }

            // The subscription this user already had. What to do about it is
            // one decision over its status, which is why it arrives as a
            // value rather than as Stripe's spelling of one.
            EventSourceStripe::SubscriptionLoaded {
                user_id,
                subscription,
            } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                // Which conversation the read carries on: `None` is a press
                // of Subscribe, `Some` a checkout, holding where its page
                // sends the window back.
                let (waiting, checkout) = match self.signup.resumed(request_id) {
                    Some(Pending::Subscribing { user_id: waiting }) => (waiting, None),

                    Some(Pending::CheckingOut {
                        user_id: waiting,
                        success_url,
                        cancel_url,
                    }) => (waiting, Some((success_url, cancel_url))),

                    _ => {
                        tracing::error!(
                            "a subscription read answered a request nobody is waiting on"
                        );

                        return;
                    }
                };

                debug_assert_eq!(
                    waiting, *user_id,
                    "a subscription read answered another user's request"
                );

                // Believed the way a webhook is, before anything is decided
                // from it. This is what repairs a webhook that never arrived:
                // somebody who paid and was never let in presses Subscribe,
                // and what Stripe says now is written and held. A read has no
                // event of its own, so it is filed under the moment it was
                // read — a webhook made before that moment is older news.
                self.entitle(
                    *user_id,
                    StateBilling::entitlement_of(subscription, self.now, self.now),
                );

                let read_id = format!("read_{}_{}", subscription.id, self.now.as_secs());

                self.write_subscription(request_id, subscription, read_id, self.now, actions);

                let subscription_id = subscription.id.as_str();
                let status = subscription.status();
                let client_secret = subscription.client_secret();

                // A checkout press. The dead statuses get a fresh page — an
                // `incomplete` left behind expires on its own inside Stripe's
                // 23 hours, having charged nothing — and anything worth
                // something means there is nothing to sell.
                if let Some((success_url, cancel_url)) = checkout {
                    match status {
                        StripeStatus::Incomplete
                        | StripeStatus::IncompleteExpired
                        | StripeStatus::Canceled => {
                            let Some(payer) = self.billing.payer(*user_id).cloned() else {
                                tracing::error!(
                                    %user_id,
                                    "a subscription was read for a user with no customer",
                                );

                                self.signup.answer(
                                    request_id,
                                    ActionRouter::Unavailable,
                                    self.now,
                                    actions,
                                );

                                return;
                            };

                            tracing::info!(
                                subscription_id,
                                status = status.as_str(),
                                "opening a checkout past a subscription that bought nothing",
                            );

                            self.signup.waits_on(
                                request_id,
                                Pending::OpeningCheckout { user_id: *user_id },
                            );

                            actions.push(Action::ToSourceStripe {
                                call_id: self.signup.calling(request_id, self.now),
                                action: ActionSourceStripe::OpenCheckout {
                                    user_id: *user_id,
                                    customer_id: payer.customer_id,
                                    success_url,
                                    cancel_url,
                                },
                            });
                        }

                        _ => {
                            tracing::info!(
                                subscription_id,
                                status = status.as_str(),
                                "asked for a checkout by somebody who already pays",
                            );

                            self.signup.answer(
                                request_id,
                                ActionRouter::Subscribed,
                                self.now,
                                actions,
                            );
                        }
                    }

                    return;
                }

                match status {
                    // Still waiting for a card, which is somebody who opened
                    // the form and walked away. They get that same invoice
                    // back — no second subscription, and nothing left behind
                    // at Stripe.
                    StripeStatus::Incomplete => {
                        self.hand_over_the_card_form(
                            request_id,
                            subscription_id,
                            client_secret,
                            actions,
                        );
                    }

                    // Dead, and it bought nothing. `incomplete_expired` is a
                    // first payment that never landed inside Stripe's 23
                    // hours; `canceled` is one that has stopped and whose
                    // period is over, because a cancellation still inside its
                    // period is worth something and lands below. Either way
                    // there is nothing to pick up, so a fresh subscription
                    // goes against the customer we already have.
                    StripeStatus::IncompleteExpired | StripeStatus::Canceled => {
                        let Some(payer) = self.billing.payer(*user_id).cloned() else {
                            tracing::error!(
                                %user_id,
                                "a subscription was read for a user with no customer",
                            );

                            self.signup.answer(
                                request_id,
                                ActionRouter::Unavailable,
                                self.now,
                                actions,
                            );

                            return;
                        };

                        tracing::info!(
                            subscription_id,
                            status = status.as_str(),
                            "replacing a subscription that bought nothing",
                        );

                        self.signup.waits_on(
                            request_id,
                            Pending::SubscribingTo {
                                user_id: *user_id,
                                customer_id: payer.customer_id.clone(),
                            },
                        );

                        actions.push(Action::ToSourceStripe {
                            call_id: self.signup.calling(request_id, self.now),
                            action: ActionSourceStripe::CreateSubscription {
                                user_id: *user_id,
                                customer_id: payer.customer_id,
                            },
                        });
                    }

                    // Everything else is a subscription worth something, and
                    // there is nothing to sell somebody who is already
                    // paying. `past_due` and `unpaid` are here on purpose:
                    // Stripe is still issuing invoices against them, so a
                    // second subscription would charge the same card twice
                    // for one app.
                    _ => {
                        tracing::info!(
                            subscription_id,
                            status = status.as_str(),
                            "asked to subscribe by somebody who already is",
                        );

                        self.signup
                            .answer(request_id, ActionRouter::Subscribed, self.now, actions);
                    }
                }
            }

            EventSourceStripe::SubscriptionCancelled {
                user_id,
                good_until,
            } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::Deleting { user_id: waiting }) = self.signup.resumed(request_id)
                else {
                    tracing::error!("a cancellation answered a request nobody is waiting on");

                    return;
                };

                debug_assert_eq!(
                    waiting, *user_id,
                    "a cancellation answered another user's request"
                );

                tracing::info!(
                    %user_id,
                    good_until = good_until.unwrap_or_default(),
                    "subscription cancelled; deleting the account it belonged to",
                );

                self.read_tokens_to_delete(request_id, *user_id, actions);
            }

            EventSourceStripe::StripeFailed { user_id, message } => {
                let Some(request_id) = self.signup.answered(call_id) else {
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

    /// Answer the route with what a card form needs, or say why there is
    /// nothing to open one with.
    ///
    /// Both doors end here — a subscription just created and one read back
    /// after somebody walked away from it — because what a waiting invoice
    /// hands over does not depend on which of them found it.
    ///
    /// No secret is a subscription with nothing to pay. On a freshly created
    /// one that means Stripe answered in a shape this build did not expect;
    /// on one read back it means the invoice it was waiting on is gone.
    /// Either way there is nothing to hand a card form, and the caller is
    /// told the machine cannot answer rather than given a secret that is not
    /// one.
    /// Hold what a provider said about a user's subscription, unless what is
    /// held came from a newer event.
    ///
    /// The same rule the store's query applies, so the copy every snapshot
    /// reads cannot disagree with the row until the next restart. Stripe says
    /// outright that events are not delivered in order.
    fn entitle(&mut self, user_id: Uuid, entitlement: Entitlement) {
        if let Some(held) = self.entitled.get(&user_id)
            && !entitlement.newer_than(held)
        {
            tracing::info!(
                %user_id,
                status = entitlement.status,
                "a delivery older than what is held says nothing new",
            );

            return;
        }

        self.entitled.insert(user_id, entitlement);
    }

    /// Write what Stripe said about a subscription, on behalf of a route.
    ///
    /// The same write for a webhook and for a read: the query refuses a
    /// delivery that is a repeat or older than what is there, so it is safe
    /// to send for everything believed. What the store keeps is the date
    /// entitlement is decided from, which is `ended_at` where a subscription
    /// stopped outright and nothing at all for one never paid for — see
    /// `StripeSubscription::good_until`.
    fn write_subscription(
        &mut self,
        request_id: RequestId,
        subscription: &src_stripe::StripeSubscription,
        event_id: String,
        event_at: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        let call_id = self.signup.calling(request_id, self.now);

        actions.push(Action::ToStore {
            call_id,
            action: ActionStore::ApplySubscriptionEvent {
                subscription_id: subscription.id.clone(),
                status: subscription.status.clone(),
                current_period_end: subscription
                    .good_until(self.now.as_secs() as i64)
                    .and_then(|secs| u64::try_from(secs).ok())
                    .map(Timestamp::from_secs),
                cancel_at_period_end: subscription.cancel_at_period_end,
                event_id,
                event_at,
            },
        });
    }

    fn hand_over_the_card_form(
        &mut self,
        request_id: RequestId,
        subscription_id: &str,
        client_secret: Option<&str>,
        actions: &mut Vec<Action>,
    ) {
        let Some(client_secret) = client_secret else {
            tracing::error!(
                subscription_id,
                "a subscription with nothing to confirm against",
            );

            self.signup
                .answer(request_id, ActionRouter::Unavailable, self.now, actions);

            return;
        };

        // Checked when the route arrived, and never taken away since.
        let Some(publishable_key) = self.billing.publishable_key() else {
            tracing::error!("a subscription was started with no publishable key to hand out");

            self.signup
                .answer(request_id, ActionRouter::Unavailable, self.now, actions);

            return;
        };

        self.signup.answer(
            request_id,
            ActionRouter::Subscribing {
                client_secret: client_secret.to_string(),
                publishable_key: publishable_key.to_string(),
            },
            self.now,
            actions,
        );
    }

    fn handle_event_source(
        &mut self,
        call_id: CallId,
        event: &EventSourcePlaid,
        actions: &mut Vec<Action>,
    ) {
        match event {
            // One bank stopped, or did not. Either way it is no longer
            // outstanding: a removal that failed is logged and does not hold
            // the deletion up, because the alternative is an account nobody
            // can delete over a bank they cannot see.
            EventSourcePlaid::ItemRemoved { item_id, removed } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::DeletingBanks { user_id, left }) =
                    self.signup.resumed(request_id)
                else {
                    tracing::error!(item_id, "a removal answered a request nobody is waiting on");

                    return;
                };

                if !removed {
                    tracing::error!(
                        item_id,
                        %user_id,
                        "a bank was not removed and is still billing; remove it by hand",
                    );
                }

                let left = left.saturating_sub(1);

                if left > 0 {
                    self.signup
                        .waits_on(request_id, Pending::DeletingBanks { user_id, left });

                    return;
                }

                self.delete_the_rows(request_id, user_id, actions);
            }

            EventSourcePlaid::WebhookKeyLoaded { kid, key } => {
                // A miss that is not written down is a call to Plaid for
                // every POST naming an id nobody has.
                self.keys.insert(
                    kid.clone(),
                    CachedKey {
                        key: key.clone(),
                        at: self.now,
                    },
                );

                let Some(request_id) = self.signup.answered(call_id) else {
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
                    call_id: self.signup.alongside(call_id, self.now),
                    action: ActionStore::SaveAccounts {
                        item_id: item_id.clone(),
                        institution_name: Some(connection.name.clone()),
                        accounts: connection.accounts.clone(),
                    },
                });

                // A sync nothing is recording is one that was given up on, so
                // there is nothing to ask Plaid next.
                let Some(cursor) = self.syncing.get(item_id).map(|s| s.cursor.clone()) else {
                    self.signup.landed(call_id, self.now, actions);

                    return;
                };

                actions.push(Action::ToSourcePlaid {
                    call_id: self.signup.passing(call_id, self.now),
                    action: ActionSourcePlaid::SyncTransactions {
                        item_id: item_id.clone(),
                        cursor,
                    },
                });
            }

            // The lists are a **diff** handed straight to the store.
            EventSourcePlaid::TransactionsLoaded {
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
                    call_id: self.signup.alongside(call_id, self.now),
                    action: ActionStore::SaveTransactions {
                        item_id: item_id.clone(),
                        added: added.clone(),
                        modified: modified.clone(),
                        removed: removed.clone(),
                        cursor: cursor.clone(),
                    },
                });

                // A sync nothing is recording is one that was given up on.
                let Some(cursor) = self.syncing.get(item_id).map(|s| s.cursor.clone()) else {
                    self.signup.landed(call_id, self.now, actions);

                    return;
                };

                let call_id = self.signup.passing(call_id, self.now);
                let item_id = item_id.clone();

                let action = if another_page {
                    ActionSourcePlaid::SyncTransactions { item_id, cursor }
                } else {
                    ActionSourcePlaid::FetchRecurring { item_id }
                };

                actions.push(Action::ToSourcePlaid { call_id, action });
            }

            EventSourcePlaid::RecurringLoaded { item_id, streams } => {
                if let Some(syncing) = self.syncing.get_mut(item_id) {
                    syncing.streams = streams.len().try_into().unwrap_or(u32::MAX);
                }

                self.sync_finished(item_id, actions);

                // The wait passes from the last call to the last write, so
                // the route is answered once the rows are there.
                actions.push(Action::ToStore {
                    call_id: self.signup.passing(call_id, self.now),
                    action: ActionStore::SaveStreams {
                        item_id: item_id.clone(),
                        streams: streams.clone(),
                    },
                });
            }

            EventSourcePlaid::LinkOpened {
                user_id,
                link_token,
                hosted_link_url,
                repair_of,
            } => {
                let Some(request_id) = self.signup.answered(call_id) else {
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
                    repair_of.as_deref(),
                    self.now,
                    actions,
                );
            }

            EventSourcePlaid::LinksPolled { polls } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::Polling { repairs }) = self.signup.resumed(request_id) else {
                    tracing::error!(call_id, "plaid answered a poll nobody asked for");

                    return;
                };

                self.signup.links_polled(
                    request_id,
                    polls,
                    &repairs,
                    &mut self.limiter,
                    self.now,
                    actions,
                );
            }

            // Named by the call rather than by the route it was made for: an
            // exchange outlives the request that asked for it, and what it
            // says has to be acted on either way.
            EventSourcePlaid::ItemLinked {
                link_token,
                item_id,
            } => {
                self.signup
                    .item_linked(call_id, link_token, item_id, self.now, actions);
            }

            EventSourcePlaid::LinkAbandoned { link_token } => {
                self.signup
                    .link_abandoned(call_id, link_token, self.now, actions);
            }

            EventSourcePlaid::PlaidFailed {
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
                    self.signup.landed(call_id, self.now, actions);

                    return;
                }

                // Not ready is not failure: a fresh Item generates its
                // history asynchronously, so the transactions are asked for
                // again on the tick and the sync stays something to watch.
                // Only until the schedule runs out — after that it is the
                // failure it reads as, and falls through with the rest.
                if *fault == PlaidFault::NotReady
                    && *stage == SyncStage::Transactions
                    && let Some(syncing) = self.syncing.get_mut(item_id)
                {
                    if let Some(wait) = NOT_READY_RETRY_SECS.get(syncing.retries as usize) {
                        syncing.retry_at = Some(self.now + Duration::from_secs(*wait));
                        syncing.retries += 1;

                        tracing::info!(item_id, wait, "transactions not ready; will ask again");

                        self.signup.landed(call_id, self.now, actions);

                        return;
                    }

                    tracing::warn!(item_id, "transactions never became ready");
                }

                self.sync_stopped(item_id, actions);

                if *fault == PlaidFault::LoginRequired {
                    actions.push(Action::ToProcess {
                        action: ActionProcess::ReportLoginRequired,
                    });

                    actions.push(Action::ToStore {
                        call_id: self.signup.passing(call_id, self.now),
                        action: ActionStore::SaveStatus {
                            item_id: item_id.clone(),
                            error_code: Some(fault.as_str().to_string()),
                        },
                    });

                    return;
                }

                // Nothing to write, so the route is not left hanging.
                self.signup.landed(call_id, self.now, actions);
            }
        }
    }

    fn handle_event_yahoo(
        &mut self,
        call_id: CallId,
        event: &EventSourceYahoo,
        actions: &mut Vec<Action>,
    ) {
        match event {
            EventSourceYahoo::ChartFetched { bars, .. } => {
                // Minted whether or not the write goes out, so a replay hands
                // out the same numbers either way.
                let save_call = self.signup.call();

                let settled = self
                    .markets
                    .chart_fetched(call_id, save_call, bars, self.now, actions);

                self.bars_settled(settled, actions);
            }

            EventSourceYahoo::ChartFailed { fault, .. } => {
                let settled = self.markets.chart_failed(call_id, fault);

                self.bars_settled(settled, actions);
            }
        }

        self.pump_markets(actions);
    }

    /// One fetch settled — landed, failed for good, or timed out — for each
    /// of these routes. A route is answered once its last one has.
    fn bars_settled(&mut self, settled: Vec<RequestId>, actions: &mut Vec<Action>) {
        for request_id in settled {
            let Some(step) = self.signup.resumed(request_id) else {
                tracing::warn!(request_id, "a fetch settled for a route already answered");

                continue;
            };

            let Pending::AwaitingBars {
                symbol,
                interval,
                start,
                end,
                left,
            } = step
            else {
                tracing::error!(
                    request_id,
                    "a fetch settled for a route not waiting on bars"
                );

                self.signup.waits_on(request_id, step);

                continue;
            };

            if left <= 1 {
                self.read_bars(request_id, symbol, interval, start, end, actions);
            } else {
                self.signup.waits_on(
                    request_id,
                    Pending::AwaitingBars {
                        symbol,
                        interval,
                        start,
                        end,
                        left: left - 1,
                    },
                );
            }
        }
    }

    /// The read a bars request is answered from, asked for only once
    /// everything the request set going has landed.
    fn read_bars(
        &mut self,
        request_id: RequestId,
        symbol: String,
        interval: src_postgres::BarInterval,
        start: Timestamp,
        end: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        self.signup.waits_on(
            request_id,
            Pending::ServingBars {
                symbol: symbol.clone(),
                interval,
            },
        );

        actions.push(Action::ToStore {
            call_id: self.signup.calling(request_id, self.now),
            action: ActionStore::LoadBars {
                source: BarSource::Yahoo,
                symbol,
                interval,
                start,
                end,
            },
        });
    }

    /// Send fetches for as long as the pace allows. Nothing new starts once
    /// the process is on its way out.
    fn pump_markets(&mut self, actions: &mut Vec<Action>) {
        if self.shutting_down() {
            return;
        }

        while self.markets.may_send(self.now) {
            let Some(fetch_id) = self.markets.next_queued() else {
                return;
            };

            let call_id = self.signup.call();

            self.markets.dispatch(fetch_id, call_id, self.now, actions);
        }
    }

    fn handle_event_process(&mut self, event: &EventProcess, actions: &mut Vec<Action>) {
        match event {
            EventProcess::Started {
                collect,
                web_origins,
                stripe_webhook_secret,
                stripe_publishable_key,
                apple_environment,
            } => {
                self.signup.started(*collect, web_origins.clone());
                self.billing.started(
                    stripe_webhook_secret.clone(),
                    stripe_publishable_key.clone(),
                    *apple_environment,
                );

                actions.push(Action::ToStore {
                    call_id: STARTUP_READ,
                    action: ActionStore::LoadSubscriptions,
                });
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

                if self.state == (Lifecycle::Starting { retry: true }) {
                    self.state = Lifecycle::Starting { retry: false };

                    actions.push(Action::ToStore {
                        call_id: STARTUP_READ,
                        action: ActionStore::LoadSubscriptions,
                    });
                }

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
                    self.retry_not_ready(actions);
                }

                // Whatever the fetch housekeeping settled is answered even on
                // the way out — a route held is a process that cannot drain —
                // and the pump guards new work itself.
                let settled = self.markets.on_tick(self.now, self.shutting_down());

                self.bars_settled(settled, actions);
                self.pump_markets(actions);
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
                            event_at: stored.last_event_at,
                        },
                    );
                }

                tracing::info!(count = subscriptions.len(), "subscriptions loaded");

                if self.starting() {
                    tracing::info!("started");

                    self.state = Lifecycle::Running;
                }
            }

            // The address a subscription's receipt goes to, which is the last
            // thing needed before Stripe can be asked for anything.
            EventStore::UserLoaded { email } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::Loading { user_id, event }) = self.signup.resumed(request_id)
                else {
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

                // The read was for the deletion code's envelope, not for
                // Stripe: the machine mints and sends the code from here.
                if matches!(event, Authenticated::DeleteCodeRequested) {
                    self.signup
                        .delete_code_requested(request_id, user_id, email, self.now, actions);

                    return;
                }

                // The same first call either way — the customer — and the
                // step notes which conversation the answer carries on.
                let step = match &event {
                    Authenticated::CheckoutRequested { origin } => {
                        // Checked when the route arrived, and the answer to
                        // an origin gone missing since is the same refusal.
                        let Some((success_url, cancel_url)) =
                            self.signup.checkout_urls(origin.as_deref())
                        else {
                            self.signup.answer(
                                request_id,
                                ActionRouter::Rejected,
                                self.now,
                                actions,
                            );

                            return;
                        };

                        Pending::CheckingOut {
                            user_id,
                            success_url,
                            cancel_url,
                        }
                    }

                    _ => Pending::Subscribing { user_id },
                };

                self.signup.waits_on(request_id, step);

                let call_id = self.signup.calling(request_id, self.now);

                self.billing
                    .subscribe_requested(user_id, email, call_id, actions);
            }

            EventStore::AccessTokensLoaded { tokens } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::DeletingReadingTokens { user_id }) =
                    self.signup.resumed(request_id)
                else {
                    tracing::error!(call_id, "tokens answered a deletion nobody asked for");

                    return;
                };

                self.remove_banks_to_delete(request_id, user_id, tokens, actions);
            }

            // The code a deletion carried, checked against the one the store
            // holds. Only an accepted code reaches the chain that cancels,
            // stops the banks and deletes the rows.
            EventStore::DeleteCodeLoaded { stored } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::DeleteSubmitted { user_id, code }) =
                    self.signup.resumed(request_id)
                else {
                    tracing::error!(call_id, "the store answered a deletion nobody submitted");

                    return;
                };

                // No code standing, and a rejection like any other: a
                // deletion is confirmed by asking for a code first.
                let Some(stored) = stored else {
                    self.signup
                        .answer(request_id, ActionRouter::Rejected, self.now, actions);

                    return;
                };

                let verdict = login_code::verify(stored, &code, self.now);

                if verdict != login_code::Verdict::Accepted {
                    // Counted whatever the reason, expiry included, for the
                    // reason a sign-in counts them.
                    actions.push(Action::ToStore {
                        call_id: self.signup.calling(request_id, self.now),
                        action: ActionStore::RecordDeleteAttempt { user_id },
                    });

                    tracing::warn!(%user_id, ?verdict, "a deletion code was rejected");

                    self.signup
                        .answer(request_id, ActionRouter::Rejected, self.now, actions);

                    return;
                }

                // The code is not spent by an action of its own: the row goes
                // with the account, and a deletion that fails part way leaves
                // it standing for the retry.
                self.delete_requested(request_id, user_id, actions);
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

                    Authenticated::RepairRequested { origin, item_id } => {
                        self.signup.repair_requested(
                            request_id,
                            user_id,
                            &item_id,
                            origin.as_deref(),
                            items,
                            &mut self.limiter,
                            self.now,
                            actions,
                        );
                    }

                    _ => unreachable!("only a snapshot or a repair asks for banks"),
                }
            }

            EventStore::PendingLinksLoaded { links } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::Loading { .. }) = self.signup.resumed(request_id) else {
                    tracing::error!(call_id, "the store answered sessions nobody asked about");

                    return;
                };

                self.signup.pending_links_loaded(
                    request_id,
                    links,
                    &mut self.limiter,
                    self.now,
                    actions,
                );
            }

            // The answer a `SESSION_FINISHED` waited on: whether its session
            // repairs an Item, which decides whether its tokens are exchanged.
            EventStore::LinkRepairLoaded { repair_of } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::WebhookLink {
                    link_token,
                    public_tokens,
                    exited,
                }) = self.signup.resumed(request_id)
                else {
                    tracing::error!(call_id, "the store answered a session nobody asked about");

                    return;
                };

                match repair_of {
                    Some(item_id) => {
                        self.signup.repair_finished(
                            request_id,
                            &link_token,
                            item_id,
                            exited,
                            self.now,
                            actions,
                        );
                    }

                    None => {
                        self.signup.session_finished(
                            request_id,
                            &link_token,
                            &public_tokens,
                            &mut self.limiter,
                            self.now,
                            actions,
                        );
                    }
                }
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

                if call_id == STARTUP_READ {
                    if self.starting() {
                        self.state = Lifecycle::Starting { retry: true };
                    }

                    return;
                }

                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                // Whatever the route was doing, it cannot do it without the
                // read.
                self.signup.resumed(request_id);

                self.signup
                    .answer(request_id, ActionRouter::Unavailable, self.now, actions);
            }

            // The coverage a bars request planned from. Whatever the plan
            // says is missing becomes fetches; nothing missing is a window
            // the store serves outright.
            EventStore::BarCoverageLoaded { spans } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::PlanningBars {
                    symbol,
                    interval,
                    start,
                    end,
                }) = self.signup.resumed(request_id)
                else {
                    tracing::error!(call_id, "the store answered coverage nobody asked about");

                    return;
                };

                let waits =
                    self.markets
                        .asked(request_id, &symbol, interval, start..end, spans, self.now);

                if waits == 0 {
                    self.read_bars(request_id, symbol, interval, start, end, actions);
                } else {
                    self.signup.waits_on(
                        request_id,
                        Pending::AwaitingBars {
                            symbol,
                            interval,
                            start,
                            end,
                            left: waits,
                        },
                    );
                }

                self.pump_markets(actions);
            }

            EventStore::BarsLoaded { bars } => {
                let Some(request_id) = self.signup.answered(call_id) else {
                    return;
                };

                let Some(Pending::ServingBars { symbol, interval }) =
                    self.signup.resumed(request_id)
                else {
                    tracing::error!(call_id, "the store answered bars nobody asked about");

                    return;
                };

                let bars = server_wire::WireBars {
                    symbol,
                    interval: interval.as_str().to_string(),
                    bars: bars
                        .iter()
                        .map(|bar| server_wire::WireBar {
                            ts: bar.ts.as_secs(),
                            open: bar.open,
                            high: bar.high,
                            low: bar.low,
                            close: bar.close,
                            volume: bar.volume,
                        })
                        .collect(),
                };

                self.signup.answer(
                    request_id,
                    ActionRouter::ServeBars { bars },
                    self.now,
                    actions,
                );
            }

            EventStore::WriteSuccess => {
                // A fetch's write, holding its waiters: theirs to settle,
                // and no route's call.
                if let Some(settled) = self.markets.store_answered(call_id, true) {
                    self.bars_settled(settled, actions);
                    self.pump_markets(actions);

                    return;
                }

                self.signup.landed(call_id, self.now, actions)
            }

            // Ends the wait: a route held for a failed write is held for
            // ever. The one exception is the write that attaches a newly
            // linked bank, which is asked for again — what it is writing down
            // is an Item slot already spent.
            EventStore::WriteError { message } => {
                tracing::error!(message, "could not write the store");

                // The window stays unknown and the waiters are answered with
                // what the store does hold; the next ask replans the gap.
                if let Some(settled) = self.markets.store_answered(call_id, false) {
                    self.bars_settled(settled, actions);
                    self.pump_markets(actions);

                    return;
                }

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
            call_id,
            action: ActionSourcePlaid::FetchBalances {
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
            PlaidEndpoint::AccountsGet,
            PlaidEndpoint::RecurringGet,
        ]
        .iter()
        .all(|endpoint| {
            self.limiter
                .can_send(now, &PlaidRateCtx::new(*endpoint, item_id))
        })
    }

    /// Ask again for the transactions a fresh bank had not generated yet.
    ///
    /// Run on the tick, which is the machine's only clock: the waits in
    /// [`NOT_READY_RETRY_SECS`] are rules, and a sleep in the runtime would
    /// be behaviour no replay of the events reproduces. A retry the budget
    /// refuses stays scheduled and the next tick asks about it again.
    fn retry_not_ready(&mut self, actions: &mut Vec<Action>) {
        let due: Vec<(String, Option<String>)> = self
            .syncing
            .iter()
            .filter(|(_, syncing)| syncing.retry_at.is_some_and(|at| at <= self.now))
            .map(|(item_id, syncing)| (item_id.clone(), syncing.cursor.clone()))
            .collect();

        for (item_id, cursor) in due {
            if !self.may_call(PlaidEndpoint::TransactionsSync, Some(&item_id)) {
                continue;
            }

            if let Some(syncing) = self.syncing.get_mut(&item_id) {
                syncing.retry_at = None;
            }

            actions.push(Action::ToSourcePlaid {
                call_id: self.signup.call(),
                action: ActionSourcePlaid::SyncTransactions { item_id, cursor },
            });
        }
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
            producing: self.signup.producing(),
            callers: self.signup.callers(),
            keys: self.keys.len(),
            request_windows: self.requests.kept(),
            plaid_windows: self.limiter.kept(),
            market_fetches: self.markets.fetches(),
            market_recent: self.markets.recent(),
        }
    }

    /// Whether the machine is still gathering who has paid.
    fn starting(&self) -> bool {
        matches!(self.state, Lifecycle::Starting { .. })
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
