//! What a believed Apple payload said about somebody's subscription.

use api_common::Timestamp;

use src_apple::AppleStatus;
use src_postgres::BillingProvider;

use crate::Entitlement;

/// One transaction, after the machine has believed it and read what it names.
///
/// The same shape from both doors — the app handing over a purchase and Apple
/// reporting a renewal — so what is done about it is written once. Which door
/// it came through is not in here, because nothing downstream decides
/// differently for it.
#[derive(Debug)]
pub struct AppleFacts {
    /// Apple's name for the payer, which outlives every renewal. This is what
    /// the row is keyed by.
    pub customer_id: String,

    /// Where Apple says the subscription stands.
    pub status: AppleStatus,

    /// The end of the period that has been paid for.
    pub good_until: Option<Timestamp>,

    /// What tells a repeat from a new fact — a transaction id from the app, a
    /// notification's id from Apple.
    pub event_id: String,

    /// When Apple signed it. The ordering guard on the write compares this,
    /// so a notification that overtook another cannot move the row backwards.
    pub event_at: Timestamp,
}

impl AppleFacts {
    /// What this entitles somebody to.
    ///
    /// `cancel_at_period_end` is always false: whether a subscription will
    /// renew lives in Apple's renewal info and not in a transaction, so
    /// anything said here would be something nothing told us.
    pub fn entitlement(&self) -> Entitlement {
        Entitlement {
            provider: BillingProvider::Apple,
            status: self.status.as_str().to_string(),
            good_until: self.good_until,
            cancel_at_period_end: false,
        }
    }
}
