//! The body of a believed webhook.
//!
//! Parsed only after [`verify`](crate::verify) has said the bytes are Stripe's.
//! Nothing here decides anything: what an event *means* for somebody's
//! entitlement is the state machine's business.

use serde::Deserialize;

use crate::StripeSubscription;

/// One event, as Stripe sends it.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeWebhookEvent {
    /// `evt_…`. The same event can arrive more than once — Stripe retries
    /// until it is answered — so this is what tells a repeat from a new fact.
    pub id: String,

    /// `customer.subscription.updated`, and the rest.
    pub r#type: String,

    /// When Stripe made it, Unix seconds. Two deliveries about the same
    /// subscription are ordered by this and not by arrival: retries and
    /// redeliveries mean the newer fact can land second.
    pub created: i64,

    pub data: StripeWebhookData,
}

/// The envelope around the object the event is about.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeWebhookData {
    pub object: StripeWebhookObject,
}

/// The object itself: a subscription, or something this app does not read.
///
/// Keyed off `object`, with a catch-all: Stripe sends events nobody asked
/// for, and an unrecognised one must be a shrug rather than a parse failure
/// that fails the whole delivery.
///
/// **The subscription is the only object entitlement is read from.** A
/// renewal, paid or failed, arrives as a `customer.subscription.updated`
/// carrying the new period and the status, so the invoice events beside it
/// say nothing this one does not — which is why the endpoint is not
/// subscribed to them. `docs/STRIPE.md` has the observed order.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "object")]
pub enum StripeWebhookObject {
    #[serde(rename = "subscription")]
    Subscription(StripeSubscription),

    #[serde(other)]
    Other,
}
