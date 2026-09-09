//! Whether somebody may use the app, and until when.

use api_common::Timestamp;

use src_postgres::BillingProvider;

/// What is known about one user's subscription.
///
/// Held on [`StateServer`](crate::StateServer) rather than inside
/// [`StateBilling`](crate::StateBilling), because it is not billing's process
/// — it is the answer several unrelated routes need. Two copies of it would be
/// two copies that can disagree about whether somebody has paid.
#[derive(Debug, Clone)]
pub struct Entitlement {
    /// Which rail proved it.
    pub provider: BillingProvider,

    /// The provider's word: `active`, `past_due`, `canceled`. **Not what
    /// decides access** — see [`Entitlement::allows`]. It is what the account
    /// screen draws, so somebody who cancelled is not told they will be
    /// billed again.
    pub status: String,

    /// The end of the period that has been paid for. `None` has never been
    /// paid for, which is not entitlement.
    pub good_until: Option<Timestamp>,

    /// Whether it stops at the end of that period.
    pub cancel_at_period_end: bool,
}

impl Entitlement {
    /// Whether this allows the app to be used, as of `now`.
    ///
    /// **The date against the clock, and nothing else.** The status is not
    /// consulted, and that is the whole of the rule:
    ///
    /// - `canceled` with a date in the future **allows**. That is what
    ///   cancelling at period end means, and `site/app/terms` promises it:
    ///   "your subscription runs to the end of the period you have paid for".
    /// - `active` with a date in the past does not. A renewal that has not
    ///   been paid leaves the status alone for a while, and a check that
    ///   believed it would serve somebody for free.
    ///
    /// Reading the status instead of the date gets both of these backwards,
    /// and the first one locks a paying customer out.
    pub fn allows(&self, now: Timestamp) -> bool {
        self.good_until
            .is_some_and(|until| now.as_secs() < until.as_secs())
    }
}
