//! Becoming a subscriber, and hearing about it afterwards.
//!
//! A process end to end, the way [`StateSignup`](crate::StateSignup) is: two
//! calls to Stripe to start a subscription, and the webhooks that say what
//! became of it. What it does *not* own is whether somebody is entitled —
//! that is [`Entitlement`](crate::Entitlement) on the parent, because a link
//! request and a snapshot both ask it and neither is billing's business.
//!
//! The two halves are asymmetric on purpose. Starting a subscription is
//! something a request asks for and waits on. Being told it was paid for
//! arrives unprompted, names no request, and is the only thing that writes
//! entitlement — a checkout that appeared to work is not a payment, because
//! the card can fail after the form closes.

use std::collections::HashMap;

use api_common::Timestamp;
use uuid::Uuid;

use src_postgres::BillingProvider;

use crate::Action;
use crate::ActionStore;
use crate::CallId;
use crate::Entitlement;

/// Starting subscriptions, and hearing what came of them.
#[derive(Debug, Default)]
pub struct StateBilling {
    /// The secret this build verifies webhooks against, read out of the
    /// environment on [`EventProcess::Started`](crate::EventProcess).
    ///
    /// `None` is a machine that has not been told the process is up, or one
    /// configured without a secret — and both refuse every webhook, which is
    /// the safe direction. An unverified webhook is somebody claiming a
    /// subscription was paid for.
    webhook_secret: Option<String>,

    /// Which user each Stripe customer is, so a webhook naming `cus_…` can be
    /// turned into a row. Populated at startup and added to as customers are
    /// created.
    ///
    /// `BTreeMap` would be the rule for anything iterated; this is only ever
    /// looked up by key, and never walked, so the order it holds is not
    /// something any action can depend on.
    customers: HashMap<String, Uuid>,

    /// The same for subscriptions, which is what most webhooks actually name.
    subscriptions: HashMap<String, Uuid>,
}

impl StateBilling {
    /// What the process came up with.
    pub fn started(&mut self, webhook_secret: Option<String>) {
        self.webhook_secret = webhook_secret;
    }

    /// Take what the store held, at startup.
    ///
    /// Both indexes come from the same rows, so they are filled together and
    /// cannot end up disagreeing about which user a subscription belongs to.
    pub fn loaded(&mut self, subscriptions: &[src_postgres::StoredSubscription]) {
        for stored in subscriptions {
            self.customers
                .insert(stored.customer_id.clone(), stored.user_id);

            if let Some(subscription_id) = &stored.subscription_id {
                self.subscriptions
                    .insert(subscription_id.clone(), stored.user_id);
            }
        }
    }

    /// Verify nothing more. The secret goes rather than a flag beside it:
    /// there is no way back from here.
    pub fn stopping(&mut self) {
        self.webhook_secret = None;
    }

    /// Whose subscription this is, if we know.
    pub fn user_for_subscription(&self, subscription_id: &str) -> Option<Uuid> {
        self.subscriptions.get(subscription_id).copied()
    }

    /// Whose customer this is, if we know.
    pub fn user_for_customer(&self, customer_id: &str) -> Option<Uuid> {
        self.customers.get(customer_id).copied()
    }

    /// Remember a customer that has just been created.
    pub fn customer_created(&mut self, customer_id: &str, user_id: Uuid) {
        self.customers.insert(customer_id.to_string(), user_id);
    }

    /// Remember a subscription that has just been created.
    pub fn subscription_created(&mut self, subscription_id: &str, user_id: Uuid) {
        self.subscriptions
            .insert(subscription_id.to_string(), user_id);
    }

    /// Whether a webhook is really Stripe's.
    ///
    /// A rule, so it is the machine's: which signature is acceptable and
    /// whether it is inside its replay window are arithmetic over what
    /// arrived, and neither needs a socket. `now` is the tick's, so a replay
    /// lands where it did the first time.
    ///
    /// Refuses without a secret, which is a machine that was never told one.
    pub fn believes(&self, body: &[u8], header: &str, now: Timestamp) -> bool {
        let Some(secret) = self.webhook_secret.as_deref() else {
            tracing::warn!("a webhook arrived at a machine with no secret to check it against");

            return false;
        };

        let Ok(signature) = src_stripe::StripeSignature::parse(header) else {
            tracing::warn!("a webhook arrived with a signature header that is not one");

            return false;
        };

        let now = now.as_secs() as i64;

        match src_stripe::verify(body, &signature, secret, now) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(%e, "a webhook was not believed");

                false
            }
        }
    }

    /// Start the conversation: a customer first, because a subscription needs
    /// one to hang off.
    ///
    /// Two calls rather than a helper doing both, because what the first
    /// answer means is a decision — and one taken where an `eval!` test can
    /// reach it.
    pub fn subscribe_requested(
        &mut self,
        user_id: Uuid,
        email: &str,
        call_id: CallId,
        actions: &mut Vec<Action>,
    ) {
        actions.push(Action::ToSourceStripe {
            action: crate::ActionSourceStripe::CreateCustomer {
                call_id,
                user_id,
                email: email.to_string(),
            },
        });
    }

    /// The customer exists. Now the subscription it pays for.
    pub fn customer_ready(
        &mut self,
        user_id: Uuid,
        customer_id: &str,
        call_id: CallId,
        actions: &mut Vec<Action>,
    ) {
        self.customer_created(customer_id, user_id);

        actions.push(Action::ToSourceStripe {
            action: crate::ActionSourceStripe::CreateSubscription {
                call_id,
                user_id,
                customer_id: customer_id.to_string(),
            },
        });
    }

    /// The subscription exists and is waiting to be paid for.
    ///
    /// The row is written here rather than when the payment lands, because the
    /// webhook that says it landed names only the subscription — without this
    /// there would be nothing to look the user up by.
    ///
    /// **No entitlement is written.** Nothing has been charged.
    pub fn subscription_ready(
        &mut self,
        user_id: Uuid,
        customer_id: &str,
        subscription_id: &str,
        status: &str,
        call_id: CallId,
        actions: &mut Vec<Action>,
    ) {
        self.subscription_created(subscription_id, user_id);

        actions.push(Action::ToStore {
            action: ActionStore::SaveCustomer {
                call_id,
                user_id,
                provider: BillingProvider::Stripe,
                customer_id: customer_id.to_string(),
                subscription_id: Some(subscription_id.to_string()),
                status: status.to_string(),
            },
        });
    }

    /// What a believed webhook says about somebody's subscription.
    ///
    /// Answers the entitlement it implies, for the parent to hold. The write
    /// to the store goes out beside it, and the ordering guard there is what
    /// makes a repeat or an out-of-order delivery harmless.
    pub fn entitlement_of(
        subscription: &src_stripe::StripeSubscription,
        now: Timestamp,
    ) -> Entitlement {
        Entitlement {
            provider: BillingProvider::Stripe,
            status: subscription.status.clone(),
            good_until: subscription
                .good_until(now.as_secs() as i64)
                .and_then(|secs| u64::try_from(secs).ok())
                .map(Timestamp::from_secs),
            cancel_at_period_end: subscription.cancel_at_period_end,
        }
    }
}
