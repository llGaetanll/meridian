//! Becoming a subscriber, and hearing about it afterwards.
//!
//! A process end to end, the way [`StateSignup`](crate::StateSignup) is: two
//! calls to Stripe to start a subscription, and the webhooks that say what
//! became of it. Apple's rail is the same process with the first half missing
//! — the App Store takes the money without asking us — so what is here is the
//! believing. What it does *not* own is whether somebody is entitled —
//! that is [`Entitlement`](crate::Entitlement) on the parent, because a link
//! request and a snapshot both ask it and neither is billing's business.
//!
//! The two halves are asymmetric on purpose. Starting a subscription is
//! something a request asks for and waits on. Being told it was paid for
//! arrives unprompted, names no request, and is the only thing that writes
//! entitlement — a checkout that appeared to work is not a payment, because
//! the card can fail after the form closes.

use std::collections::BTreeMap;

use api_common::Timestamp;
use uuid::Uuid;

use src_apple::AppleEnvironment;
use src_apple::AppleNotification;
use src_apple::AppleStatus;
use src_apple::AppleTransaction;
use src_postgres::BillingProvider;

use crate::Action;
use crate::ActionStore;
use crate::AppleFacts;
use crate::CallId;
use crate::Entitlement;

/// The app a purchase has to have been made in.
///
/// Ours and fixed, rather than configured: a build that could be told another
/// bundle id is a build that can be talked into believing somebody else's
/// receipts.
const BUNDLE_ID: &str = "com.meridian.phone";

/// The one thing this app sells.
const PRODUCT_ID: &str = "com.meridian.phone.monthly";

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

    /// What a card form opens Stripe.js with, handed out beside each secret
    /// it confirms against. Not a credential: Stripe makes it to be in a page.
    publishable_key: Option<String>,

    /// Which user each Stripe customer is, so a webhook naming `cus_…` can be
    /// turned into a row. Populated at startup and added to as customers are
    /// created.
    customers: BTreeMap<String, Uuid>,

    /// The same for subscriptions, which is what most webhooks actually name.
    subscriptions: BTreeMap<String, Uuid>,

    /// Which App Store this build sells in, read out of the environment on
    /// [`EventProcess::Started`](crate::EventProcess).
    ///
    /// `None` is a machine that has not been told, and it refuses every Apple
    /// payload. There is no default and nothing is inferred: a sandbox
    /// purchase costs nothing, so a build that took one for a real
    /// subscription would be giving the app away to anybody with Xcode.
    apple_environment: Option<AppleEnvironment>,
}

impl StateBilling {
    /// What the process came up with.
    pub fn started(
        &mut self,
        webhook_secret: Option<String>,
        publishable_key: Option<String>,
        apple_environment: Option<AppleEnvironment>,
    ) {
        self.webhook_secret = webhook_secret;
        self.publishable_key = publishable_key;
        self.apple_environment = apple_environment;
    }

    /// What a card form opens Stripe.js with. `None` is a process that can
    /// sell nothing on the web.
    pub fn publishable_key(&self) -> Option<&str> {
        self.publishable_key.as_deref()
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

    /// Verify nothing more. What was needed to believe anything goes, rather
    /// than a flag beside it: there is no way back from here.
    pub fn stopping(&mut self) {
        self.webhook_secret = None;
        self.apple_environment = None;
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

    /// Which subscription is this user's, if this process knows of one.
    ///
    /// The reverse of [`user_for_subscription`](Self::user_for_subscription),
    /// walked rather than indexed: it is asked once when an account is being
    /// deleted, and a second map would be a second thing to keep in step.
    pub fn subscription_for_user(&self, user_id: Uuid) -> Option<String> {
        self.subscriptions
            .iter()
            .find(|(_, owner)| **owner == user_id)
            .map(|(subscription_id, _)| subscription_id.clone())
    }

    /// Forget everything about a user whose account is gone.
    ///
    /// Both indexes, because a webhook arriving after the account was deleted
    /// must not find a user id that no longer names anybody.
    pub fn forget(&mut self, user_id: Uuid) {
        self.customers.retain(|_, owner| *owner != user_id);
        self.subscriptions.retain(|_, owner| *owner != user_id);
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
            call_id,
            action: crate::ActionSourceStripe::CreateCustomer {
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
            call_id,
            action: crate::ActionSourceStripe::CreateSubscription {
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
            call_id,
            action: ActionStore::SaveCustomer {
                user_id,
                provider: BillingProvider::Stripe,
                customer_id: customer_id.to_string(),
                subscription_id: Some(subscription_id.to_string()),
                status: status.to_string(),
            },
        });
    }

    /// Whether a payload the app handed over is really Apple's, and what it
    /// says.
    ///
    /// A rule, so it is the machine's: which certificate chain is acceptable
    /// and which store this build sells in are both arithmetic over what
    /// arrived, and neither needs a socket. `now` is the tick's, so a replay
    /// lands where it did the first time.
    ///
    /// `None` is refused, for one of the reasons logged inside. The caller
    /// gets no say in which: every way of being wrong is the same answer to
    /// whoever sent it.
    pub fn believes_apple(&self, signed: &str, now: Timestamp) -> Option<AppleFacts> {
        let transaction = self.transaction_of(signed, now)?;

        Some(AppleFacts {
            status: status_of(&transaction, now),
            good_until: seconds(transaction.good_until()),
            event_id: transaction.transaction_id.clone(),
            event_at: stamp(transaction.signed_date, now),
            customer_id: transaction.original_transaction_id,
        })
    }

    /// The same for a notification Apple sent, which carries its transaction
    /// signed again inside it.
    ///
    /// **The inner payload is believed in its own right.** A notification
    /// vouches for its own bytes and for nothing it quotes, so the transaction
    /// goes through the same chain walk as one the app hands over — and a
    /// notification naming a sandbox transaction is refused on a production
    /// build even though the envelope verified.
    pub fn believes_apple_notification(&self, signed: &str, now: Timestamp) -> Option<AppleFacts> {
        let payload = self.verified(signed, now)?;

        let Ok(notification) = serde_json::from_slice::<AppleNotification>(&payload) else {
            tracing::warn!("a believed notification was not one this build understands");

            return None;
        };

        // The ones that name nobody: a renewal date extended across many
        // subscriptions at once. Believed and acted on by nothing.
        let Some(data) = notification.data else {
            tracing::info!(
                kind = notification.notification_type,
                "a notification about no one subscription",
            );

            return None;
        };

        let Some(inner) = data.signed_transaction_info else {
            tracing::warn!(
                kind = notification.notification_type,
                "a notification carrying no transaction",
            );

            return None;
        };

        let transaction = self.transaction_of(&inner, now)?;

        Some(AppleFacts {
            // Apple's own word where it said one, and the least the
            // transaction can be read as where it did not.
            status: data
                .status
                .map(AppleStatus::from_code)
                .unwrap_or_else(|| status_of(&transaction, now)),

            good_until: seconds(transaction.good_until()),

            // The notification's id and not the transaction's: Apple retries a
            // delivery for days, and this is what tells the second one from a
            // new fact.
            event_id: notification.notification_uuid,

            event_at: stamp(transaction.signed_date, now),
            customer_id: transaction.original_transaction_id,
        })
    }

    /// One transaction, believed and checked against what this build sells.
    ///
    /// Four things have to hold, and the first is the one that matters most: a
    /// sandbox purchase is free, so a production build that took one would be
    /// giving the app away to anybody with Xcode.
    fn transaction_of(&self, signed: &str, now: Timestamp) -> Option<AppleTransaction> {
        let payload = self.verified(signed, now)?;

        let Ok(transaction) = serde_json::from_slice::<AppleTransaction>(&payload) else {
            tracing::warn!("a believed payload was not a transaction this build understands");

            return None;
        };

        let sells_in = self.apple_environment?;

        if AppleEnvironment::from_name(&transaction.environment) != Some(sells_in) {
            tracing::warn!(
                said = transaction.environment,
                sells_in = sells_in.as_str(),
                "a transaction from another App Store than this build sells in",
            );

            return None;
        }

        if transaction.bundle_id != BUNDLE_ID {
            tracing::warn!(
                bundle_id = transaction.bundle_id,
                "a transaction from another app",
            );

            return None;
        }

        if transaction.product_id != PRODUCT_ID {
            tracing::warn!(
                product_id = transaction.product_id,
                "a transaction for something this app does not sell",
            );

            return None;
        }

        Some(transaction)
    }

    /// The payload of something Apple signed, or nothing.
    ///
    /// Refuses without an environment, which is a machine that was never told
    /// which store it sells in — checked here rather than after the chain
    /// walk, so a build that cannot act on a payload does not spend three
    /// signature checks finding out.
    fn verified(&self, signed: &str, now: Timestamp) -> Option<Vec<u8>> {
        if self.apple_environment.is_none() {
            tracing::warn!("an apple payload arrived at a machine that sells in no store");

            return None;
        }

        match src_apple::verify(signed, src_apple::APPLE_ROOT_CA_G3, now.as_secs() as i64) {
            Ok(payload) => Some(payload),
            Err(e) => {
                tracing::warn!(%e, "an apple payload was not believed");

                None
            }
        }
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

/// What a transaction alone says the subscription is.
///
/// A transaction carries no status — Apple puts that in the notification
/// beside it — so this is the least its own dates can be read as. Restoring a
/// purchase that lapsed months ago says `expired` rather than `active`, which
/// is the difference between a paywall that explains itself and one that
/// claims a webhook went missing.
fn status_of(transaction: &AppleTransaction, now: Timestamp) -> AppleStatus {
    if transaction.revocation_date.is_some() {
        return AppleStatus::Revoked;
    }

    match transaction.good_until() {
        Some(until) if until > now.as_secs() as i64 => AppleStatus::Active,
        _ => AppleStatus::Expired,
    }
}

/// A date Apple sent, as the rest of the server counts time.
fn seconds(value: Option<i64>) -> Option<Timestamp> {
    value
        .and_then(|secs| u64::try_from(secs).ok())
        .map(Timestamp::from_secs)
}

/// The same for a date in milliseconds, falling back to the tick's own clock
/// for one that is not a time at all.
fn stamp(millis: i64, now: Timestamp) -> Timestamp {
    u64::try_from(millis / 1000)
        .map(Timestamp::from_secs)
        .unwrap_or(now)
}
