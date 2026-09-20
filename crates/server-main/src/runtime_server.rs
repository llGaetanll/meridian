//! The IO half of the server: everything effectful, and nothing that decides.
//!
//! Only [`RuntimeServer::run`] is public, so nothing can do IO without an
//! action asking for it.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use api_common::Timestamp;
use tokio::sync::mpsc;

use crate::Action;
use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionSocket;
use crate::ActionSourcePlaid;
use crate::ActionSourceStripe;
use crate::ActionStore;
use crate::CallId;
use crate::EmailSend;
use crate::Event;
use crate::EventProcess;
use crate::EventSourcePlaid;
use crate::EventSourceStripe;
use crate::EventStore;
use crate::EventTerminal;
use crate::LinkOpener;
use crate::OauthRedirect;
use crate::PlaidCollect;
use crate::PublicUrl;
use crate::StateServer;
use crate::StoredCode;
use crate::WebOrigins;
use crate::project;
use crate::runtime_plaid;
use crate::runtime_router::ActionRx;
use crate::runtime_router::ActionTx;
use crate::runtime_router::Answer;
use crate::runtime_router::EventRx;
use crate::runtime_router::EventTx;
use crate::runtime_router::RuntimeRouter;
use crate::runtime_stripe;
use crate::source_pool::SourcePool;
use server_wire::SyncStage;
use server_wire::WireSnapshot;
use src_apple::AppleEnvironment;
use src_plaid::PlaidClient;
use src_plaid::PlaidCredentials;
use src_plaid::PlaidError;
use src_plaid::PlaidFault;
use src_postgres::Db;
use src_postgres::DbKey;
use src_resend::ResendClient;
use src_resend::ResendCredentials;
use src_stripe::StripeClient;
use src_stripe::StripeCredentials;
use uuid::Uuid;

/// How often the machine is handed fresh entropy. Not about running out — it
/// bounds how much a single leaked seed is worth.
const RESEED_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

/// How many calls to Plaid may be in flight at once. Plaid's per-Item rate
/// limit is the machine's problem; this only bounds outstanding work.
const PLAID_AT_ONCE: usize = 4;

/// How many sign-in emails may be in flight at once.
const EMAIL_AT_ONCE: usize = 2;

/// Calls to Stripe at once.
///
/// Small, because they only ever come from somebody at a card form: two calls
/// per subscription, at whatever rate people sign up.
const STRIPE_AT_ONCE: usize = 2;

/// How often the machine is told what time it is. A second, far finer than the
/// shortest window the machine measures.
const TICK_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// How many writes may be in flight at once. Stops a burst queueing inside the
/// Postgres pool instead, where it is invisible.
const STORE_AT_ONCE: usize = 8;

pub struct RuntimeServer {
    /// The store. The access tokens live in it, so a server without one has no
    /// way to reach any bank and nothing to serve.
    db: Db,

    /// What the access token column is sealed with. Read on the way up, so a
    /// key that is missing or the wrong length is a server that does not start.
    db_key: DbKey,

    /// Where an answer goes; the router is on another task.
    action_tx: ActionTx,

    /// Every call to Plaid, made on the one client so they share a connection
    /// pool. One per process, and the only place the environment is decided.
    plaid_client: PlaidClient,

    /// Stripe, where the process was configured for it.
    ///
    /// Optional so that a server without Stripe still starts and still serves
    /// everybody who is already entitled. What it cannot do is start a new
    /// subscription, and the route says so rather than the process refusing to
    /// come up.
    stripe_client: Option<StripeClient>,

    /// How a sign-in code reaches the person asking for it.
    email_send: EmailSend,

    /// Resend, when [`RuntimeServer::email_send`] says to use it. `None` when
    /// it says `log`.
    resend: Option<ResendClient>,

    /// Every Item's access token, by `item_id`, read from the store at boot
    /// and added to as banks are linked.
    ///
    /// Here rather than in the machine: a token is a live bank credential and
    /// the machine is logged and replayed, so it never sees one. Actions name
    /// an Item and the runtime looks the token up.
    tokens: RwLock<HashMap<String, String>>,

    /// Where Plaid should call, or `None` when nothing should.
    ///
    /// `Some` exactly when `PLAID_COLLECT=webhook`, where it is required and
    /// validated on the way up, so a server that cannot name itself will not
    /// start.
    webhook: Option<String>,

    /// Where an OAuth bank's app returns the user to ours, or `None` where
    /// there is nowhere for it to return to yet.
    handback: Option<OauthRedirect>,

    /// Every call to Plaid.
    plaid: SourcePool,

    /// Every call to Stripe. Its own pool for the same reason email has one:
    /// a bank that has gone quiet must not use up the room a payment needs.
    stripe: SourcePool,

    /// Every sign-in email. Its own pool so a slow bank cannot use up the room
    /// a sign-in needs.
    email: SourcePool,

    /// Every write. The machine holds a route's answer until the store reports.
    store: SourcePool,
}

impl RuntimeServer {
    /// Build the server, wire the signal handler, and serve until told to stop.
    pub async fn run() -> anyhow::Result<()> {
        // Read before anything is served, not on the first request.
        let plaid_client = runtime_plaid::client(PlaidCredentials::from_env()?)?;

        // Read at startup like everything else, and not fatal: a key that is
        // not one is a server that cannot sell, which is worth saying out loud
        // rather than dying over.
        // A fact from the edge, reported rather than acted on: whether a
        // webhook can be believed is the machine's decision.
        let stripe_webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET")
            .ok()
            .filter(|secret| !secret.is_empty());

        if stripe_webhook_secret.is_none() {
            tracing::warn!("no stripe webhook secret: every stripe webhook will be refused");
        }

        // Said out loud or not at all, the way `PLAID_ENV` is. A fact from the
        // edge: whether a purchase can be believed is the machine's decision,
        // and a build that has not said which store it sells in refuses every
        // one of them rather than guessing at the harmless-looking answer.
        let apple_environment = std::env::var("APPLE_ENV")
            .ok()
            .as_deref()
            .and_then(AppleEnvironment::from_setting);

        match apple_environment {
            Some(environment) => tracing::info!(
                store = environment.as_str(),
                "selling through the app store",
            ),
            None => tracing::warn!(
                "APPLE_ENV is not `sandbox` or `production`: every purchase will be refused",
            ),
        }

        let stripe_client = match StripeCredentials::from_env() {
            Ok(credentials) => match runtime_stripe::client(credentials) {
                Ok(client) => Some(client),
                Err(e) => {
                    tracing::error!(error = %e, "stripe is configured and would not build");

                    None
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "no stripe: nobody can start a subscription");

                None
            }
        };

        // Handed to the card form, so it has to be the same mode as the key
        // the subscription was made with: a form opened with the other mode's
        // key cannot confirm the secret it was given, and the failure is in
        // the browser with nothing in this log.
        let stripe_publishable_key = std::env::var("STRIPE_PUBLISHABLE_KEY")
            .ok()
            .filter(|key| !key.is_empty())
            .filter(|key| match stripe_client.as_ref() {
                Some(client) => {
                    match StripeCredentials::check_key("STRIPE_PUBLISHABLE_KEY", key, client.env())
                    {
                        Ok(()) => true,
                        Err(e) => {
                            tracing::error!(error = %e, "the publishable key is refused");

                            false
                        }
                    }
                }
                None => true,
            });

        if stripe_publishable_key.is_none() {
            tracing::warn!("no stripe publishable key: nothing can be sold on the web");
        }
        let collect = PlaidCollect::from_env()?;

        // Required in webhook mode and meaningless in poll mode. Read here so
        // a server that cannot name its own address says so at startup.
        let public_url = match collect {
            PlaidCollect::Webhook => Some(PublicUrl::from_env()?),
            PlaidCollect::Poll => None,
        };

        // Optional, unlike everything else read here. A deployment whose app
        // cannot claim a Universal Link has no address to give a bank, and the
        // link token says so rather than claiming one.
        let handback = OauthRedirect::from_env()?;

        let db = Self::open_store().await?;

        let db_key = DbKey::from_env()?;

        let email_send = EmailSend::from_env()?;

        // The one misconfiguration this process can never notice on its own: a
        // browser refused by CORS never sends the request at all.
        let origins = WebOrigins::from_env()?;

        // A missing key is a server that does not start, not a sign-in that
        // fails.
        let resend = match email_send {
            EmailSend::Resend => Some(ResendClient::new(&ResendCredentials::from_env()?)?),
            EmailSend::Log => None,
        };

        if email_send == EmailSend::Log {
            tracing::warn!(
                "EMAIL_SEND=log: sign-in codes are written to this log and no email is sent",
            );
        }

        let webhook = public_url.as_ref().map(PublicUrl::webhook);

        tracing::info!(
            env = plaid_client.env().as_str(),
            collect = collect.as_str(),
            webhook = webhook.as_deref().unwrap_or("none"),
            email = email_send.as_str(),
            origins = origins.as_str(),
            handback = handback.as_ref().map_or("none", OauthRedirect::as_str),
            "serving live plaid data",
        );

        // Unbounded deliberately: a pool that had to wait to report would be
        // holding an answer the loop is about to be asked for. Each pool's own
        // room is what bounds the work.
        let (event_tx, mut event_rx): (EventTx, EventRx) = mpsc::unbounded_channel();

        let (action_tx, action_rx): (ActionTx, ActionRx) = mpsc::unbounded_channel();

        let router_events = event_tx.clone();

        let web_origins = origins.clone();

        tokio::spawn(async move {
            if let Err(e) = RuntimeRouter::run(action_rx, router_events, origins).await {
                tracing::error!(error = %e, "the router stopped");
            }
        });

        let runtime = Arc::new(Self {
            plaid: SourcePool::new("plaid", PLAID_AT_ONCE, event_tx.clone()),
            stripe: SourcePool::new("stripe", STRIPE_AT_ONCE, event_tx.clone()),
            email: SourcePool::new("email", EMAIL_AT_ONCE, event_tx.clone()),
            store: SourcePool::new("store", STORE_AT_ONCE, event_tx.clone()),
            action_tx,
            plaid_client,
            stripe_client,
            email_send,
            resend,
            tokens: RwLock::new(HashMap::new()),
            webhook,
            handback,
            db,
            db_key,
        });

        // Owned outright by the loop below, which is what makes `handle_events`
        // an ordinary `&mut` call rather than one taken under a guard.
        let mut state = StateServer::new();

        let mut clock = tokio::time::interval(TICK_EVERY);
        let mut reseed = tokio::time::interval(RESEED_EVERY);

        // Reused every turn. One allocation for the life of the process.
        let mut events: Vec<Event> = Vec::new();
        let mut actions: Vec<Action> = Vec::new();

        // The clock is first: the machine starts at the epoch, and a code
        // minted before the first tick would expire in 1970.
        for event in [
            EventProcess::Tick { now: unix_now() },
            EventProcess::Started {
                collect,
                web_origins,
                stripe_webhook_secret: stripe_webhook_secret.clone(),
                stripe_publishable_key: stripe_publishable_key.clone(),
                apple_environment,
            },
            EventProcess::Seeded { seed: fresh_seed() },
        ] {
            let _ = event_tx.send(Event::FromProcess { event });
        }

        loop {
            tokio::select! {
                _ = clock.tick() => events.push(Event::FromProcess {
                    event: EventProcess::Tick { now: unix_now() },
                }),

                // What bounds the damage of a leaked log: a seed tells you the
                // codes drawn before the next one and no more.
                _ = reseed.tick() => events.push(Event::FromProcess {
                    event: EventProcess::Seeded { seed: fresh_seed() },
                }),

                Some(event) = event_rx.recv() => events.push(event),

                // The signal is not the decision: the machine says what
                // happens next, which is what makes shutdown replayable.
                event = signal_received() => events.push(Event::FromTerminal { event }),
            }

            // Every branch above pushes exactly one event, so this is only
            // reachable if one of them stops doing so.
            if events.is_empty() {
                continue;
            }

            state.handle_events(&events, &mut actions);

            tracing::debug!(?events, ?actions, "turn");

            runtime.perform_actions(&mut actions);

            events.clear();

            // Not `Stopping`: what is outstanding when a signal arrives still
            // has to come back or be given up on.
            if state.stopped() {
                break;
            }
        }

        tracing::info!("server stopped");

        Ok(())
    }

    /// Carry out everything the machine decided this turn.
    ///
    /// **Nothing here waits.** Every call goes to the pool for its source and
    /// comes back as an event, which keeps one slow bank out of other turns.
    fn perform_actions(self: &Arc<Self>, actions: &mut Vec<Action>) {
        for action in actions.drain(..) {
            self.perform_action(action);
        }
    }

    /// Set one action going. Which pool is the only thing decided here, so a
    /// slow source cannot crowd out the others.
    fn perform_action(self: &Arc<Self>, action: Action) {
        match action {
            // By now everything the answer was waiting on has landed: the
            // machine held it until the last write reported.
            Action::ToRouter { request_id, action } => {
                let answer = Answer::ToRoute { request_id, action };

                if self.action_tx.send(answer).is_err() {
                    tracing::error!(request_id, "the router is gone; its answer was dropped");
                }
            }

            // Down the same channel as an answer, because a push decided in
            // the same turn as the answer that opened the stream must not
            // overtake it.
            Action::ToSocket {
                action: ActionSocket::Send { conn_id, status },
            } => {
                let answer = Answer::ToSocket { conn_id, status };

                if self.action_tx.send(answer).is_err() {
                    tracing::error!(conn_id, "the router is gone; a push was dropped");
                }
            }

            Action::ToSourcePlaid { .. } => self.calling(&self.plaid, action),

            Action::ToSourceStripe { .. } => self.calling(&self.stripe, action),

            Action::ToEmail { .. } => self.calling(&self.email, action),

            Action::ToStore { .. } => self.calling(&self.store, action),

            Action::ToProcess { ref action } => self.perform_action_process(action),
        }
    }

    /// Hand one action to a pool, to be carried out whenever there is room.
    fn calling(self: &Arc<Self>, pool: &SourcePool, action: Action) {
        let runtime = Arc::clone(self);

        pool.call(async move { runtime.carry_out(&action).await });
    }

    /// Connect to Postgres and bring the schema up to date.
    ///
    /// Migrations run on boot rather than as a script somebody has to
    /// remember. A missing or wrong `DATABASE_URL` is fatal: the access tokens
    /// live here, so a server without a store cannot reach a bank at all.
    async fn open_store() -> anyhow::Result<Db> {
        let url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL is not set"))?;

        let db = src_postgres::connect(&url).await?;

        src_postgres::migrate(&db).await?;

        tracing::info!(store = "postgres", "store ready");

        Ok(db)
    }

    /// Carry one action out, and report what came of it.
    ///
    /// Whatever it produces is an *event*, never another action: deciding what
    /// to do about an answer is the machine's job.
    async fn carry_out(&self, action: &Action) -> Vec<Event> {
        match action {
            Action::ToSourcePlaid { call_id, action } => {
                return self.perform_action_source(*call_id, action).await;
            }
            Action::ToSourceStripe { call_id, action } => {
                return self.perform_action_stripe(*call_id, action).await;
            }
            Action::ToStore { call_id, action } => {
                return self.perform_action_store(*call_id, action).await;
            }
            Action::ToEmail { action } => self.perform_action_email(action).await,
            Action::ToRouter { .. } | Action::ToSocket { .. } | Action::ToProcess { .. } => {
                unreachable!("delivered rather than performed")
            }
        }

        Vec::new()
    }

    /// Carry out one call to Stripe, and report what came of it.
    ///
    /// The runtime chooses *how*, never whether: a process without Stripe
    /// reports that as a failed call, and the machine decides what to tell the
    /// caller. Reading `self.stripe_client` to pick a different event would be
    /// behaviour no `eval!` test can reach.
    async fn perform_action_stripe(
        &self,
        call_id: CallId,
        action: &ActionSourceStripe,
    ) -> Vec<Event> {
        let user_id = match action {
            ActionSourceStripe::CreateCustomer { user_id, .. }
            | ActionSourceStripe::CreateSubscription { user_id, .. }
            | ActionSourceStripe::OpenCheckout { user_id, .. }
            | ActionSourceStripe::LoadSubscription { user_id, .. }
            | ActionSourceStripe::CancelSubscription { user_id, .. } => *user_id,
        };

        let Some(client) = self.stripe_client.as_ref() else {
            return vec![Event::FromSourceStripe {
                call_id,
                event: EventSourceStripe::StripeFailed {
                    user_id,
                    message: "this server has no stripe configured".to_string(),
                },
            }];
        };

        let event = match action {
            ActionSourceStripe::CreateCustomer { user_id, email } => {
                match runtime_stripe::create_customer(client, *user_id, email).await {
                    Ok(customer_id) => EventSourceStripe::CustomerCreated {
                        user_id: *user_id,
                        customer_id,
                    },
                    Err(e) => EventSourceStripe::StripeFailed {
                        user_id: *user_id,
                        message: e.to_string(),
                    },
                }
            }

            ActionSourceStripe::CancelSubscription {
                user_id,
                subscription_id,
                now,
            } => match runtime_stripe::cancel_subscription(
                client,
                subscription_id,
                now.as_secs() as i64,
            )
            .await
            {
                Ok(good_until) => EventSourceStripe::SubscriptionCancelled {
                    user_id: *user_id,
                    good_until: good_until.and_then(|secs| u64::try_from(secs).ok()),
                },
                Err(e) => EventSourceStripe::StripeFailed {
                    user_id: *user_id,
                    message: e.to_string(),
                },
            },

            ActionSourceStripe::LoadSubscription {
                user_id,
                subscription_id,
            } => match runtime_stripe::get_subscription(client, subscription_id).await {
                Ok(subscription) => EventSourceStripe::SubscriptionLoaded {
                    user_id: *user_id,
                    subscription,
                },
                Err(e) => EventSourceStripe::StripeFailed {
                    user_id: *user_id,
                    message: e.to_string(),
                },
            },

            ActionSourceStripe::OpenCheckout {
                user_id,
                customer_id,
                success_url,
                cancel_url,
            } => {
                match runtime_stripe::create_checkout(client, customer_id, success_url, cancel_url)
                    .await
                {
                    Ok(url) => EventSourceStripe::CheckoutOpened {
                        user_id: *user_id,
                        url,
                    },
                    Err(e) => EventSourceStripe::StripeFailed {
                        user_id: *user_id,
                        message: e.to_string(),
                    },
                }
            }

            ActionSourceStripe::CreateSubscription {
                user_id,
                customer_id,
            } => match runtime_stripe::create_subscription(client, customer_id).await {
                Ok(subscription) => EventSourceStripe::SubscriptionCreated {
                    user_id: *user_id,
                    subscription_id: subscription.id,
                    status: subscription.status,
                    client_secret: subscription.client_secret,
                },
                Err(e) => EventSourceStripe::StripeFailed {
                    user_id: *user_id,
                    message: e.to_string(),
                },
            },
        };

        vec![Event::FromSourceStripe { call_id, event }]
    }

    /// Get a code to a person, or say where it went instead.
    async fn perform_action_email(&self, action: &ActionEmail) {
        let (email, code, subject, text) = match action {
            ActionEmail::SendCode { email, code } => (
                email,
                code,
                format!("{code} is your Meridian code"),
                format!(
                    "Your Meridian sign-in code is {code}.\n\n\
                     It works for the next 10 minutes. If you did not ask for it, \
                     you can ignore this — nobody can sign in without it.",
                ),
            ),

            ActionEmail::SendDeleteCode { email, code } => (
                email,
                code,
                format!("{code} is your Meridian account deletion code"),
                format!(
                    "Your Meridian account deletion code is {code}.\n\n\
                     Entering it deletes your account, your banks and your \
                     transactions permanently. It works for the next 10 minutes. \
                     If you did not ask to delete your account, you can ignore \
                     this — nothing is deleted without it.",
                ),
            ),
        };

        match (&self.email_send, &self.resend) {
            (EmailSend::Resend, Some(client)) => {
                let message = src_resend::Email::new(client.from(), email, subject, text);

                if let Err(e) = src_resend::send(client, &message).await {
                    tracing::error!(error = %e, "could not send a code");
                }
            }

            // Loud on purpose: a server accidentally running this way says so
            // on every single sign-in rather than looking healthy.
            _ => tracing::warn!(
                email,
                code,
                "EMAIL_SEND=log: no email sent, the code is on this line",
            ),
        }
    }

    /// Everything Plaid is asked for.
    async fn perform_action_source(
        &self,
        call_id: CallId,
        action: &ActionSourcePlaid,
    ) -> Vec<Event> {
        match action {
            ActionSourcePlaid::FetchBalances { item_id, now } => {
                self.fetch_balances(call_id, item_id, *now).await
            }

            ActionSourcePlaid::SyncTransactions { item_id, cursor } => {
                self.sync_transactions(call_id, item_id, cursor.clone())
                    .await
            }

            ActionSourcePlaid::FetchRecurring { item_id } => {
                self.fetch_recurring(call_id, item_id).await
            }

            ActionSourcePlaid::RemoveItem {
                item_id,
                access_token,
            } => self.remove_item(call_id, item_id, access_token).await,

            ActionSourcePlaid::FetchWebhookKey { kid } => {
                self.fetch_webhook_key(call_id, kid).await
            }

            ActionSourcePlaid::OpenLink { user_id, opener } => {
                self.open_link(call_id, *user_id, opener).await
            }

            ActionSourcePlaid::OpenRepair {
                user_id,
                item_id,
                opener,
            } => self.open_repair(call_id, *user_id, item_id, opener).await,

            ActionSourcePlaid::PollLinks { link_tokens } => {
                self.poll_links(call_id, link_tokens).await
            }

            ActionSourcePlaid::ExchangePublicToken {
                link_token,
                public_token,
            } => {
                self.exchange_public_token(call_id, link_token, public_token)
                    .await
            }
        }
    }

    /// Ask Plaid for one webhook signing key, and report what came back.
    ///
    /// A key that will not fetch is reported as absent: remembering that keeps
    /// an invented `kid` from costing a call every time it is sent.
    async fn fetch_webhook_key(&self, call_id: CallId, kid: &str) -> Vec<Event> {
        let key = match runtime_plaid::webhook_key(&self.plaid_client, kid).await {
            Ok(key) => Some(key),

            Err(e) => {
                tracing::warn!(kid, error = %e, "could not fetch a webhook key");

                None
            }
        };

        vec![Event::FromSourcePlaid {
            call_id,
            event: EventSourcePlaid::WebhookKeyLoaded {
                kid: kid.to_string(),
                key,
            },
        }]
    }

    /// Open a Link session and report the page that hosts it.
    async fn open_link(&self, call_id: CallId, user_id: Uuid, opener: &LinkOpener) -> Vec<Event> {
        let handback = self.handback.as_ref().filter(|_| opener.hands_back());

        let event = match runtime_plaid::open_link(
            &self.plaid_client,
            &user_id.to_string(),
            &opener.completion_redirect(),
            self.webhook.as_deref(),
            handback,
        )
        .await
        {
            Ok((link_token, Some(hosted_link_url))) => EventSourcePlaid::LinkOpened {
                user_id,
                link_token,
                hosted_link_url,
                repair_of: None,
            },

            // A link token with no hosted URL means the request did not ask for
            // Hosted Link, which it always does.
            Ok((link_token, None)) => {
                tracing::error!("plaid answered a link token with no hosted url");

                EventSourcePlaid::LinkAbandoned { link_token }
            }

            Err(e) => {
                tracing::warn!(error = %e, "could not open a link session");

                EventSourcePlaid::LinkAbandoned {
                    link_token: String::new(),
                }
            }
        };

        vec![Event::FromSourcePlaid { call_id, event }]
    }

    /// Open an update-mode Link session against one bank, and report the page
    /// that hosts it.
    ///
    /// The access token is looked up here and goes no further: the action
    /// names the Item, and the credential stays beside the others.
    async fn open_repair(
        &self,
        call_id: CallId,
        user_id: Uuid,
        item_id: &str,
        opener: &LinkOpener,
    ) -> Vec<Event> {
        let Some(access_token) = self.token_for(item_id) else {
            tracing::error!(item_id, "asked to repair a bank with no token held for it");

            return vec![Event::FromSourcePlaid {
                call_id,
                event: EventSourcePlaid::LinkAbandoned {
                    link_token: String::new(),
                },
            }];
        };

        let handback = self.handback.as_ref().filter(|_| opener.hands_back());

        let event = match runtime_plaid::open_repair(
            &self.plaid_client,
            &user_id.to_string(),
            &access_token,
            &opener.completion_redirect(),
            self.webhook.as_deref(),
            handback,
        )
        .await
        {
            Ok((link_token, Some(hosted_link_url))) => EventSourcePlaid::LinkOpened {
                user_id,
                link_token,
                hosted_link_url,
                repair_of: Some(item_id.to_string()),
            },

            Ok((link_token, None)) => {
                tracing::error!("plaid answered a link token with no hosted url");

                EventSourcePlaid::LinkAbandoned { link_token }
            }

            Err(e) => {
                tracing::warn!(error = %e, "could not open a repair session");

                EventSourcePlaid::LinkAbandoned {
                    link_token: String::new(),
                }
            }
        };

        vec![Event::FromSourcePlaid { call_id, event }]
    }

    /// Ask what became of every Link session the machine named: one call each,
    /// and one event for all of them.
    async fn poll_links(&self, call_id: CallId, link_tokens: &[String]) -> Vec<Event> {
        let mut polls = Vec::with_capacity(link_tokens.len());

        for link_token in link_tokens {
            let poll = match runtime_plaid::poll_link(&self.plaid_client, link_token).await {
                Ok((finished, public_tokens)) => crate::LinkPoll {
                    link_token: link_token.clone(),
                    finished,
                    public_tokens,
                },

                Err(e) => {
                    tracing::warn!(error = %e, "could not poll a link session");

                    // Not abandoned: a failed poll says nothing about the
                    // session, and giving up would lose a real connection.
                    crate::LinkPoll {
                        link_token: link_token.clone(),
                        finished: false,
                        public_tokens: Vec::new(),
                    }
                }
            };

            polls.push(poll);
        }

        vec![Event::FromSourcePlaid {
            call_id,
            event: EventSourcePlaid::LinksPolled { polls },
        }]
    }

    /// Trade a `public_token` for the permanent one.
    ///
    /// **The one call in this server whose effect cannot be undone.** In
    /// Production it spends an Item slot that `/item/remove` does not give
    /// back, which is why it carries the narrowest budget of the seven
    /// endpoints and why the write behind it is retried rather than dropped.
    ///
    /// The access token goes into the runtime's map and no further: an event
    /// carrying one would put a bank credential in the log.
    async fn exchange_public_token(
        &self,
        call_id: CallId,
        link_token: &str,
        public_token: &str,
    ) -> Vec<Event> {
        let event = match runtime_plaid::exchange(&self.plaid_client, public_token).await {
            Ok((item_id, access_token)) => {
                self.write_tokens().insert(item_id.clone(), access_token);

                EventSourcePlaid::ItemLinked {
                    link_token: link_token.to_string(),
                    item_id,
                }
            }

            Err(e) => {
                tracing::warn!(error = %e, "could not exchange a public token");

                EventSourcePlaid::LinkAbandoned {
                    link_token: link_token.to_string(),
                }
            }
        };

        vec![Event::FromSourcePlaid { call_id, event }]
    }

    /// Everything the store is asked for, as the event that says what came of it.
    ///
    /// The machine holds a route's answer until this arrives, so a write that
    /// reported nothing would be a request that never returns — which is why a
    /// failure is an event too.
    async fn perform_action_store(&self, call_id: CallId, action: &ActionStore) -> Vec<Event> {
        // A read answers with what it found; a write answers that it landed.
        let event = match action {
            ActionStore::LoadSession {
                token_sha256, now, ..
            } => self.load_session(token_sha256, *now).await,

            ActionStore::LoadItem { item_id, .. } => EventStore::ItemLoaded {
                item: self.item_facts(item_id).await,
            },

            ActionStore::LoadItems { user_id, .. } => self.load_items(*user_id).await,

            ActionStore::LoadPendingLinks { user_id, .. } => {
                self.load_pending_links(*user_id).await
            }

            ActionStore::LoadLinkRepair { link_token } => self.load_link_repair(link_token).await,

            ActionStore::LoadSnapshot { user_id, .. } => EventStore::SnapshotLoaded {
                snapshot: self.project_snapshot(*user_id).await,
            },

            ActionStore::LoadCode { email, .. } => self.load_code(email).await,

            ActionStore::LoadDeleteCode { user_id } => self.load_delete_code(*user_id).await,

            ActionStore::LoadSubscriptions => self.load_subscriptions().await,

            ActionStore::LoadUser { user_id, .. } => self.load_user(*user_id).await,

            ActionStore::LoadAccessTokens { user_id, .. } => {
                self.load_access_tokens(*user_id).await
            }

            ActionStore::SaveItem {
                item_id,
                link_token,
                ..
            } => match self.save_linked_item(&self.db, item_id, link_token).await {
                Ok(item) => EventStore::ItemSaved { item },

                Err(e) => EventStore::WriteError {
                    message: e.to_string(),
                },
            },

            _ => match self.write_store(action).await {
                Ok(()) => EventStore::WriteSuccess,

                Err(e) => EventStore::WriteError {
                    message: e.to_string(),
                },
            },
        };

        vec![Event::FromStore { call_id, event }]
    }

    /// Who a session token signs in, as the store holds it.
    ///
    /// The clock comes from the action: the row this matches is the one the
    /// machine reckoned was still good.
    async fn load_session(&self, token_sha256: &[u8], now: Timestamp) -> EventStore {
        match src_postgres::user_for_session(&self.db, token_sha256, now).await {
            Ok(user_id) => EventStore::SessionResolved { user_id },

            Err(e) => EventStore::ReadError {
                message: e.to_string(),
            },
        }
    }

    /// Which banks are this user's, and where each one's last sync reached.
    async fn load_items(&self, user_id: Uuid) -> EventStore {
        match self.facts_for(user_id).await {
            Ok(items) => EventStore::ItemsLoaded { items },

            Err(e) => EventStore::ReadError {
                message: e.to_string(),
            },
        }
    }

    async fn load_pending_links(&self, user_id: Uuid) -> EventStore {
        match src_postgres::pending_links(&self.db, user_id).await {
            Ok(links) => EventStore::PendingLinksLoaded {
                links: links
                    .into_iter()
                    .map(|link| crate::PendingLinkFacts {
                        link_token: link.link_token,
                        repair_of: link.repair_of,
                    })
                    .collect(),
            },

            Err(e) => EventStore::ReadError {
                message: e.to_string(),
            },
        }
    }

    /// Which Item a Link session repairs, as the store holds it.
    async fn load_link_repair(&self, link_token: &str) -> EventStore {
        match src_postgres::repair_for_link(&self.db, link_token).await {
            Ok(repair_of) => EventStore::LinkRepairLoaded { repair_of },

            Err(e) => EventStore::ReadError {
                message: e.to_string(),
            },
        }
    }

    /// What code stands against an address, as the store holds it.
    /// Every subscription, for the read at startup.
    async fn load_subscriptions(&self) -> EventStore {
        match src_postgres::load_subscriptions(&self.db).await {
            Ok(subscriptions) => EventStore::SubscriptionsLoaded { subscriptions },

            Err(e) => EventStore::ReadError {
                message: e.to_string(),
            },
        }
    }

    /// Every access token this user's banks were reached with, opened.
    ///
    /// The read that has to happen before anything is deleted: `items` goes
    /// with the user, and a token nobody holds is a bank that bills forever.
    async fn load_access_tokens(&self, user_id: uuid::Uuid) -> EventStore {
        match src_postgres::load_access_tokens(&self.db, user_id, &self.db_key).await {
            Ok(tokens) => EventStore::AccessTokensLoaded { tokens },

            Err(e) => EventStore::ReadError {
                message: e.to_string(),
            },
        }
    }

    /// The address a subscription's receipt goes to.
    async fn load_user(&self, user_id: uuid::Uuid) -> EventStore {
        match src_postgres::load_email(&self.db, user_id).await {
            Ok(email) => EventStore::UserLoaded { email },

            Err(e) => EventStore::ReadError {
                message: e.to_string(),
            },
        }
    }

    async fn load_code(&self, email: &str) -> EventStore {
        match src_postgres::load_code(&self.db, email).await {
            Ok(stored) => EventStore::CodeLoaded {
                stored: stored.map(|s| StoredCode {
                    code_sha256: s.code_sha256,
                    expires_at: s.expires_at,
                    attempts: s.attempts,
                }),
            },

            Err(e) => EventStore::ReadError {
                message: e.to_string(),
            },
        }
    }

    /// What deletion code stands against a user, as the store holds it.
    async fn load_delete_code(&self, user_id: uuid::Uuid) -> EventStore {
        match src_postgres::load_delete_code(&self.db, user_id).await {
            Ok(stored) => EventStore::DeleteCodeLoaded {
                stored: stored.map(|s| StoredCode {
                    code_sha256: s.code_sha256,
                    expires_at: s.expires_at,
                    attempts: s.attempts,
                }),
            },

            Err(e) => EventStore::ReadError {
                message: e.to_string(),
            },
        }
    }

    /// The write itself; `perform_action_store` turns it into an event.
    async fn write_store(&self, action: &ActionStore) -> Result<(), src_postgres::PostgresError> {
        let db = &self.db;

        match action {
            ActionStore::SaveCode {
                email,
                code_sha256,
                expires_at,
            } => {
                src_postgres::save_code(db, email, code_sha256, *expires_at).await?;
            }

            ActionStore::SaveCustomer {
                user_id,
                provider,
                customer_id,
                subscription_id,
                status,
            } => {
                src_postgres::save_customer(
                    db,
                    *user_id,
                    *provider,
                    customer_id,
                    subscription_id.as_deref(),
                    status,
                )
                .await?;
            }

            ActionStore::ApplySubscriptionEvent {
                subscription_id,
                status,
                current_period_end,
                cancel_at_period_end,
                event_id,
                event_at,
            } => {
                // The query refuses a repeat or a delivery older than what is
                // there, and says which it did. Reported rather than dropped:
                // a webhook that changed nothing is worth seeing in a log next
                // to the one that did.
                let moved = src_postgres::apply_subscription_event(
                    db,
                    subscription_id,
                    status,
                    *current_period_end,
                    *cancel_at_period_end,
                    event_id,
                    *event_at,
                )
                .await?;

                if !moved {
                    tracing::info!(
                        subscription_id,
                        event_id,
                        "a webhook said nothing new about a subscription"
                    );
                }
            }

            ActionStore::ApplyAppleEvent {
                user_id,
                customer_id,
                status,
                current_period_end,
                event_id,
                event_at,
            } => {
                // The same conditional write as the one above, keyed by the
                // customer, and reported the same way: a delivery that changed
                // nothing is worth seeing in a log next to one that did.
                let moved = src_postgres::apply_apple_event(
                    db,
                    *user_id,
                    customer_id,
                    status,
                    *current_period_end,
                    event_id,
                    *event_at,
                )
                .await?;

                if !moved {
                    tracing::info!(
                        customer_id,
                        event_id,
                        "a purchase said nothing new about a subscription"
                    );
                }
            }

            ActionStore::DeleteUser { user_id } => {
                src_postgres::delete_user(db, *user_id).await?;
            }

            ActionStore::RecordAttempt { email } => {
                src_postgres::record_attempt(db, email).await?;
            }

            ActionStore::SaveDeleteCode {
                user_id,
                code_sha256,
                expires_at,
            } => {
                src_postgres::save_delete_code(db, *user_id, code_sha256, *expires_at).await?;
            }

            ActionStore::RecordDeleteAttempt { user_id } => {
                src_postgres::record_delete_attempt(db, *user_id).await?;
            }

            ActionStore::DeleteCode { email } => {
                src_postgres::delete_code(db, email).await?;
            }

            ActionStore::CreateSession {
                email,
                token_sha256,
                expires_at,
            } => {
                // An address becomes a user here, which is what makes there be
                // no separate sign-up and no way to choose wrong.
                let created = async {
                    let user_id = src_postgres::upsert_user(db, email).await?;

                    src_postgres::create_session(db, token_sha256, user_id, Some(*expires_at))
                        .await?;

                    tracing::info!(%user_id, "signed in");

                    Ok::<(), src_postgres::PostgresError>(())
                };

                created.await?;
            }

            ActionStore::SweepSessions { now } => {
                let gone = src_postgres::sweep_sessions(db, *now).await?;

                // Silent when there was nothing to drop, which is most hours.
                if gone > 0 {
                    tracing::info!(sessions = gone, "swept expired sessions");
                }
            }

            ActionStore::SaveLink {
                link_token,
                user_id,
                repair_of,
            } => {
                src_postgres::save_link(db, link_token, *user_id, repair_of.as_deref()).await?;
            }

            ActionStore::CollectLink { link_token } => {
                src_postgres::collect_link(db, link_token).await?;
            }

            ActionStore::SaveAccounts { .. }
            | ActionStore::SaveTransactions { .. }
            | ActionStore::SaveStreams { .. }
            | ActionStore::SaveStatus { .. } => {
                self.write(db, action).await?;
            }

            ActionStore::LoadSession { .. }
            | ActionStore::LoadItem { .. }
            | ActionStore::LoadItems { .. }
            | ActionStore::LoadPendingLinks { .. }
            | ActionStore::LoadLinkRepair { .. }
            | ActionStore::LoadSnapshot { .. }
            | ActionStore::LoadCode { .. }
            | ActionStore::LoadDeleteCode { .. }
            | ActionStore::LoadSubscriptions
            | ActionStore::LoadUser { .. }
            | ActionStore::LoadAccessTokens { .. }
            | ActionStore::SaveItem { .. } => {
                unreachable!("answered by `perform_action_store`, which says what it wrote")
            }
        }

        Ok(())
    }

    /// Write a newly linked bank, and say what was written.
    ///
    /// The owner comes from the Link session rather than from the action,
    /// which is what stops one account collecting a bank another connected.
    /// It is also the whole reason this answers rather than reporting that it
    /// landed: the first sync starts here, and a sync has to know whose it is.
    ///
    /// `None` for the two ways there is nothing to write — no token held and
    /// no session to attach it to — because neither leaves anything to sync.
    async fn save_linked_item(
        &self,
        db: &Db,
        item_id: &str,
        link_token: &str,
    ) -> Result<Option<crate::ItemFacts>, src_postgres::PostgresError> {
        let Some(access_token) = self.token_for(item_id) else {
            tracing::error!(item_id, "a bank was linked with no token held for it");

            return Ok(None);
        };

        let Some(user_id) = src_postgres::user_for_link(db, link_token).await? else {
            tracing::error!(link_token, "a bank was linked against no session");

            return Ok(None);
        };

        src_postgres::save_item(
            db,
            &self.db_key,
            &src_postgres::StoredItem {
                item_id: item_id.to_string(),
                user_id,
                access_token,
                institution_id: None,
                institution_name: None,
                cursor: None,
                error_code: None,
                synced_at: Timestamp::EPOCH,
            },
        )
        .await?;

        tracing::info!(item_id, %user_id, "bank linked");

        Ok(Some(crate::ItemFacts {
            item_id: item_id.to_string(),
            user_id,
            // A bank attached seconds ago has synced nothing and has no cursor
            // to resume from.
            cursor: None,
            synced_at: None,
            login_required: false,
        }))
    }

    /// One user's banks, as the store holds them.
    ///
    /// The connections are assembled here rather than in the store, because a
    /// connection is a *projection* — initials, a worded status — and
    /// `src-postgres` returns records.
    async fn connections_for(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<api_ledger::Connection>, src_postgres::PostgresError> {
        let items = src_postgres::load_items(&self.db, &self.db_key, user_id).await?;

        let mut connections = Vec::with_capacity(items.len());

        for item in &items {
            let accounts = src_postgres::load_accounts(&self.db, &item.item_id).await?;

            let name = item
                .institution_name
                .clone()
                .or_else(|| item.institution_id.clone())
                .unwrap_or_else(|| "Bank".to_string());

            connections.push(api_ledger::Connection {
                id: item.item_id.clone(),
                initials: api_ledger::initials(&name),
                name,
                status: project::connection_status(item.error_code.as_deref(), item.synced_at),
                accounts,
            });
        }

        Ok(connections)
    }

    /// One bank, by the id Plaid names it with, and its token remembered.
    ///
    /// `None` covers both a bank the store has no row for and a store that
    /// would not answer: the machine has no cursor to sync from either way.
    async fn item_facts(&self, item_id: &str) -> Option<crate::ItemFacts> {
        let item = match src_postgres::load_item(&self.db, &self.db_key, item_id).await {
            Ok(item) => item?,

            Err(e) => {
                tracing::error!(item_id, error = %e, "could not read the bank a webhook names");

                return None;
            }
        };

        // Into the runtime's map, so a fetch can find it without going back.
        self.write_tokens()
            .insert(item.item_id.clone(), item.access_token.clone());

        Some(crate::ItemFacts {
            item_id: item.item_id,
            user_id: item.user_id,
            cursor: item.cursor,
            synced_at: Some(item.synced_at),
            login_required: item.error_code.as_deref() == Some(src_plaid::ITEM_LOGIN_REQUIRED),
        })
    }

    /// The facts the machine needs to decide whether to sync each of them.
    async fn facts_for(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<crate::ItemFacts>, src_postgres::PostgresError> {
        let items = src_postgres::load_items(&self.db, &self.db_key, user_id).await?;

        // Into the runtime's map, so a fetch need not go back to the database.
        {
            let mut tokens = self.write_tokens();

            for item in &items {
                tokens.insert(item.item_id.clone(), item.access_token.clone());
            }
        }

        Ok(items
            .iter()
            .map(|item| crate::ItemFacts {
                item_id: item.item_id.clone(),
                user_id: item.user_id,
                cursor: item.cursor.clone(),
                synced_at: Some(item.synced_at),
                login_required: item.error_code.as_deref() == Some("ITEM_LOGIN_REQUIRED"),
            })
            .collect())
    }

    /// The writes, in the order a failure can be recovered from.
    ///
    /// The cursor goes **last**. If anything before it fails the cursor stays
    /// put and the next sync re-reads the same range, which is safe because
    /// applying a diff twice is a no-op.
    async fn write(
        &self,
        db: &Db,
        action: &ActionStore,
    ) -> Result<(), src_postgres::PostgresError> {
        match action {
            ActionStore::SaveAccounts {
                item_id,
                institution_name,
                accounts,
            } => {
                // The Item row already exists, so this is the institution name
                // catching up; `user_id` is deliberately not touched.
                src_postgres::name_item(db, item_id, institution_name.as_deref()).await?;

                src_postgres::save_accounts(db, item_id, accounts).await?;
            }

            ActionStore::SaveTransactions {
                item_id,
                added,
                modified,
                removed,
                cursor,
            } => {
                src_postgres::apply_entries(db, item_id, added, modified, removed).await?;

                // Last and separately, so a page that half-wrote is read again
                // rather than skipped.
                src_postgres::save_cursor(db, item_id, cursor).await?;

                tracing::debug!(
                    item_id,
                    added = added.len(),
                    modified = modified.len(),
                    removed = removed.len(),
                    "written to the store",
                );
            }

            ActionStore::SaveStreams { item_id, streams } => {
                src_postgres::save_streams(db, item_id, streams).await?;
            }

            ActionStore::SaveStatus {
                item_id,
                error_code,
            } => {
                src_postgres::save_error(db, item_id, error_code.as_deref()).await?;
            }

            // Carried out where they are decided. `write` is the sync path,
            // whose ordering is load-bearing in a way these are not.
            ActionStore::LoadSession { .. }
            | ActionStore::LoadItem { .. }
            | ActionStore::LoadItems { .. }
            | ActionStore::LoadPendingLinks { .. }
            | ActionStore::LoadLinkRepair { .. }
            | ActionStore::LoadSnapshot { .. }
            | ActionStore::LoadCode { .. }
            | ActionStore::SaveCode { .. }
            | ActionStore::RecordAttempt { .. }
            | ActionStore::DeleteCode { .. }
            | ActionStore::LoadDeleteCode { .. }
            | ActionStore::SaveDeleteCode { .. }
            | ActionStore::RecordDeleteAttempt { .. }
            | ActionStore::CreateSession { .. }
            | ActionStore::SweepSessions { .. }
            | ActionStore::SaveLink { .. }
            | ActionStore::CollectLink { .. }
            | ActionStore::SaveItem { .. }
            | ActionStore::SaveCustomer { .. }
            | ActionStore::ApplySubscriptionEvent { .. }
            | ActionStore::ApplyAppleEvent { .. }
            | ActionStore::LoadSubscriptions
            | ActionStore::LoadUser { .. }
            | ActionStore::LoadAccessTokens { .. }
            | ActionStore::DeleteUser { .. } => {}
        }

        Ok(())
    }

    fn perform_action_process(&self, action: &ActionProcess) {
        match action {
            // Nothing to carry out. The action exists because a runtime with
            // something to wind down would wind it down on this.
            ActionProcess::Shutdown => {}

            ActionProcess::ReportLoginRequired => tracing::warn!(
                "a connection needs the user to sign in again; syncs will fail until they do",
            ),
        }
    }

    /// Stop one bank at Plaid, and say whether it stopped.
    ///
    /// A failure is reported rather than retried: what to do about a bank that
    /// is still billing is the machine's decision, and the log line is what
    /// says to go and look.
    async fn remove_item(&self, call_id: CallId, item_id: &str, access_token: &str) -> Vec<Event> {
        let removed = match runtime_plaid::remove_item(&self.plaid_client, access_token).await {
            Ok(()) => true,
            Err(e) => {
                tracing::error!(
                    item_id,
                    error = %e,
                    "an item was not removed at plaid, and is still billing",
                );

                false
            }
        };

        vec![Event::FromSourcePlaid {
            call_id,
            event: EventSourcePlaid::ItemRemoved {
                item_id: item_id.to_string(),
                removed,
            },
        }]
    }

    /// Ask Plaid what one bank's accounts hold, and feed the answer back in.
    ///
    /// Nothing here inspects it, and nothing here decides what happens next;
    /// that is what makes the loop closed.
    async fn fetch_balances(&self, call_id: CallId, item_id: &str, now: Timestamp) -> Vec<Event> {
        let Some(access_token) = self.token_for(item_id) else {
            return vec![no_token(call_id, item_id, SyncStage::Accounts)];
        };

        let event =
            match runtime_plaid::fetch_balances(&self.plaid_client, &access_token, now).await {
                Ok(connection) => EventSourcePlaid::BalancesLoaded {
                    item_id: item_id.to_string(),
                    connection,
                },

                Err(e) => failed(item_id, SyncStage::Accounts, &e),
            };

        vec![Event::FromSourcePlaid { call_id, event }]
    }

    /// Ask Plaid for one page of what has changed on a bank.
    async fn sync_transactions(
        &self,
        call_id: CallId,
        item_id: &str,
        cursor: Option<String>,
    ) -> Vec<Event> {
        let Some(access_token) = self.token_for(item_id) else {
            return vec![no_token(call_id, item_id, SyncStage::Transactions)];
        };

        let event =
            match runtime_plaid::sync_transactions(&self.plaid_client, &access_token, cursor).await
            {
                Ok(page) => EventSourcePlaid::TransactionsLoaded {
                    item_id: item_id.to_string(),
                    added: page.added,
                    modified: page.modified,
                    removed: page.removed,
                    cursor: page.cursor,
                    has_more: page.has_more,
                },

                Err(e) => failed(item_id, SyncStage::Transactions, &e),
            };

        vec![Event::FromSourcePlaid { call_id, event }]
    }

    /// Ask Plaid which charges on a bank come back.
    async fn fetch_recurring(&self, call_id: CallId, item_id: &str) -> Vec<Event> {
        let Some(access_token) = self.token_for(item_id) else {
            return vec![no_token(call_id, item_id, SyncStage::Streams)];
        };

        let event = match runtime_plaid::fetch_recurring(&self.plaid_client, &access_token).await {
            Ok(streams) => EventSourcePlaid::RecurringLoaded {
                item_id: item_id.to_string(),
                streams,
            },

            Err(e) => failed(item_id, SyncStage::Streams, &e),
        };

        vec![Event::FromSourcePlaid { call_id, event }]
    }

    /// Everything one user knows, as of now, read from the store.
    async fn project_snapshot(&self, user_id: Uuid) -> WireSnapshot {
        let entries = src_postgres::load_entries(&self.db, user_id).await;
        let streams = src_postgres::load_streams(&self.db, user_id).await;
        let connections = self.connections_for(user_id).await;

        // A store that will not answer is served around: an empty ledger is
        // what a first run has, and a better answer than one needing a screen.
        if let Err(e) = entries.as_ref().err().map_or(Ok(()), Err) {
            tracing::error!(error = %e, "could not read the ledger");
        }

        WireSnapshot::new(
            &entries.unwrap_or_default(),
            &connections.unwrap_or_default(),
            &streams.unwrap_or_default(),
        )
    }

    /// One Item's access token, if we hold one.
    fn token_for(&self, item_id: &str) -> Option<String> {
        match self.tokens.read() {
            Ok(tokens) => tokens.get(item_id).cloned(),
            Err(poisoned) => {
                tracing::error!(
                    "the token map lock is poisoned — a thread panicked while holding it; \
                     carrying on with the held data",
                );

                poisoned.into_inner().get(item_id).cloned()
            }
        }
    }

    fn write_tokens(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<String, String>> {
        match self.tokens.write() {
            Ok(tokens) => tokens,
            Err(poisoned) => {
                tracing::error!(
                    "the token map lock is poisoned — a thread panicked while holding it; \
                     carrying on with the held data",
                );

                poisoned.into_inner()
            }
        }
    }
}

/// 32 bytes from the operating system: the one place entropy enters, and it is
/// at the edge — the machine has no source of its own, as it has no clock.
fn fresh_seed() -> [u8; 32] {
    rand::random()
}

/// Resolves with whichever signal arrives first.
async fn signal_received() -> EventTerminal {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            // Without a handler there is nothing to wait for; ^C still works.
            Err(e) => {
                tracing::warn!(error = %e, "could not listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = interrupt => EventTerminal::SigInt,
        _ = terminate => EventTerminal::SigTerm,
    }
}

/// The wall clock, read here and nowhere below. The state machine takes it as
/// an argument so that a replay is not at the mercy of when it happens to run.
fn unix_now() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| Timestamp::from_secs(d.as_secs()))
        .unwrap_or(Timestamp::EPOCH)
}

/// The machine named a bank the runtime holds no token for, so the call cannot
/// be made at all.
///
/// The machine reads its Items off its own connection list, each built from a
/// stored Item, so this is the two of them having drifted.
fn no_token(call_id: CallId, item_id: &str, stage: SyncStage) -> Event {
    tracing::error!(item_id, "asked to call plaid about an item with no token");

    Event::FromSourcePlaid {
        call_id,
        event: EventSourcePlaid::PlaidFailed {
            item_id: item_id.to_string(),
            stage,
            fault: PlaidFault::NoToken,
        },
    }
}

/// What one refused call reaches the machine as.
fn failed(item_id: &str, stage: SyncStage, error: &PlaidError) -> EventSourcePlaid {
    let fault = PlaidFault::of(error);

    tracing::warn!(
        item_id,
        stage = server_wire::stage_name(stage),
        code = fault.as_str(),
        error = %error,
        "a plaid call failed",
    );

    EventSourcePlaid::PlaidFailed {
        item_id: item_id.to_string(),
        stage,
        fault,
    }
}
