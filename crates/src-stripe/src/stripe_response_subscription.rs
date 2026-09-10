//! What the subscription endpoints answer with.

use serde::Deserialize;

/// One subscription, cut down to what decides entitlement.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeSubscription {
    /// `sub_…`.
    pub id: String,

    /// `incomplete`, `active`, `past_due`, `canceled`, and the rest. A string
    /// rather than an enum: Stripe adds statuses, and a new one arriving must
    /// not be a parse failure in the middle of a webhook.
    pub status: String,

    /// `cus_…`, so a subscription read on its own still says whose it is.
    pub customer: String,

    /// Whether it stops at the end of the period already paid for.
    #[serde(default)]
    pub cancel_at_period_end: bool,

    /// The lines it bills. Exactly one here — Meridian sells one thing — but
    /// Stripe models it as a list and the period lives on the entry.
    pub items: StripeSubscriptionItems,

    /// The unpaid invoice and the secret its card form confirms against.
    ///
    /// **Either an id or the whole object**, which is how Stripe answers every
    /// expandable field: a call that asked to expand it gets the object, and
    /// one that did not gets `"in_…"`. A type insisting on the object parses
    /// the create — which expands it — and then fails on every plain read of
    /// the same subscription.
    #[serde(default)]
    pub latest_invoice: Option<StripeExpandable<StripeInvoice>>,
}

/// A field Stripe will send either as an id or as the object itself.
///
/// Untagged, so serde tries the object first and falls back to the string.
/// Order matters: a string would never parse as the struct, but the struct arm
/// must be tried first or an object would be read as neither.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StripeExpandable<T> {
    /// The object, from a call that asked for it.
    Expanded(T),

    /// Just its id, from a call that did not.
    Id(String),
}

impl<T> StripeExpandable<T> {
    /// The object, where this call asked for it.
    pub fn expanded(&self) -> Option<&T> {
        match self {
            StripeExpandable::Expanded(value) => Some(value),
            StripeExpandable::Id(_) => None,
        }
    }
}

impl StripeSubscription {
    /// When the period being paid for runs out, as a Unix timestamp.
    ///
    /// **Not a top-level field.** It sits on the item, and reading it off the
    /// subscription answers `None` for every subscription there is — which
    /// reads as "expired" to anything deciding entitlement.
    pub fn current_period_end(&self) -> Option<i64> {
        self.items.data.first().map(|item| item.current_period_end)
    }

    /// The secret the card form needs, if there is an invoice waiting to be
    /// paid. `None` once the subscription is settled.
    pub fn client_secret(&self) -> Option<&str> {
        self.latest_invoice
            .as_ref()?
            .expanded()?
            .confirmation_secret
            .as_ref()
            .map(|secret| secret.client_secret.as_str())
    }
}

/// Stripe's list envelope, of which only the entries matter.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeSubscriptionItems {
    pub data: Vec<StripeSubscriptionItem>,
}

/// One billed line, and the period it covers.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeSubscriptionItem {
    pub current_period_start: i64,
    pub current_period_end: i64,
}

/// The invoice a subscription is waiting on.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeInvoice {
    pub id: String,

    /// Only there when the call asked for it with
    /// `expand[]=latest_invoice.confirmation_secret`.
    #[serde(default)]
    pub confirmation_secret: Option<StripeConfirmationSecret>,
}

/// What the card form confirms against, without the card ever reaching us.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeConfirmationSecret {
    pub client_secret: String,
}
