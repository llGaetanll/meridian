//! What the subscription endpoints answer with.

use serde::Deserialize;

use crate::StripeStatus;

/// One subscription, cut down to what decides entitlement.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeSubscription {
    /// `sub_…`.
    pub id: String,

    /// `incomplete`, `active`, `past_due`, `canceled`, and the rest. A string
    /// rather than an enum: Stripe adds statuses, and a new one arriving must
    /// not be a parse failure in the middle of a webhook. What one means is
    /// [`StripeSubscription::status`].
    pub status: String,

    /// `cus_…`, so a subscription read on its own still says whose it is.
    pub customer: String,

    /// Whether it stops at the end of the period already paid for.
    #[serde(default)]
    pub cancel_at_period_end: bool,

    /// When the subscription ended outright, if it did.
    ///
    /// **The difference between the two ways a subscription stops.** Cancelled
    /// at period end, this is null and the period runs on — access continues,
    /// which is what the terms promise. Ended for real (deleted, or Stripe
    /// gave up retrying a payment) it is set, and `current_period_end` still
    /// reports the period it had reached: a month in the future, for a
    /// subscription that stopped a moment ago.
    #[serde(default)]
    pub ended_at: Option<i64>,

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

    /// What Stripe's word for this subscription means.
    pub fn status(&self) -> StripeStatus {
        StripeStatus::from_name(&self.status)
    }

    /// Whether this status is one that stops being worth anything at once,
    /// whatever period it reports.
    ///
    /// Stripe is explicit about `unpaid`: *"Revoke access to your product when
    /// the subscription is unpaid, because payments were already attempted and
    /// retried while past_due."* It keeps generating invoices while nobody
    /// pays them, so the period rolls forward on a subscription earning
    /// nothing — and `ended_at` stays null, because it has not ended.
    ///
    /// `incomplete_expired` is the same shape at the other end of the life: a
    /// first payment that never landed within 23 hours. Terminal, and it never
    /// bought anything.
    ///
    /// `past_due` is deliberately **not** here. Stripe is still retrying, the
    /// customer has paid for the period they are in, and the ordinary reason
    /// is an expired card on somebody who will replace it. Revoking there
    /// takes the app away from a paying customer mid-month over a card that a
    /// smart retry fixes an hour later.
    fn status_ends_it(&self) -> bool {
        matches!(
            self.status(),
            StripeStatus::Unpaid | StripeStatus::IncompleteExpired
        )
    }

    /// When this subscription stops being worth anything, or `None` for one
    /// that has never been worth anything.
    ///
    /// Four cases, and the reason this is not just `current_period_end`:
    ///
    /// - **`incomplete`.** Started and not paid for. Stripe reports the period
    ///   a month out from the moment it is created, before any card, and
    ///   believing that opens the app for somebody who has paid nothing. So
    ///   there is no date at all.
    /// - **Cancelled at period end.** `ended_at` is null while it runs, so the
    ///   period end stands and access continues to the day paid for — which
    ///   is what `site/app/terms` promises.
    /// - **Ended outright**, deleted or cancelled immediately. `ended_at` is
    ///   the moment it stopped. The period end still reports the period it
    ///   never finished, a month out, and believing that hands a deleted
    ///   subscription another month.
    /// - **`unpaid` or `incomplete_expired`.** Nothing ended and nothing was
    ///   paid: `ended_at` is null and the period may even have rolled forward.
    ///   Worth nothing from now, so `now` is the answer — which the caller
    ///   supplies, because a machine that read a clock here would be doing IO.
    pub fn good_until(&self, now: i64) -> Option<i64> {
        if self.status() == StripeStatus::Incomplete {
            return None;
        }

        if self.status_ends_it() {
            return Some(now);
        }

        match self.ended_at {
            Some(ended_at) => Some(ended_at),
            None => self.current_period_end(),
        }
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
