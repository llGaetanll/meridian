//! Whether somebody may use the app, and until when.

use api_common::Timestamp;

use server_wire::UnpaidReason;
use server_wire::WireUnpaid;
use server_wire::unpaid_reason_name;
use src_postgres::BillingProvider;
use src_stripe::StripeStatus;

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

    /// When the event this was read from was made. What tells a stale
    /// delivery from a new fact: the store's query refuses anything older
    /// than the row's last event, and [`Entitlement::newer_than`] is the same
    /// rule held here, so the two copies cannot disagree. `None` is a row
    /// written before anything was.
    pub event_at: Option<Timestamp>,
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

    /// Whether this is at least as new as `held`, and so replaces it.
    ///
    /// `<=` rather than `<`, the same as the query: Stripe's `created` is
    /// accurate to the second, and two events sharing a second both land.
    /// Anything undated is older than everything.
    pub fn newer_than(&self, held: &Entitlement) -> bool {
        match (self.event_at, held.event_at) {
            (Some(mine), Some(theirs)) => theirs.as_secs() <= mine.as_secs(),
            (Some(_), None) => true,
            (None, _) => false,
        }
    }
}

/// Why a session is being refused, for the paywall the client draws.
///
/// The one place a provider's vocabulary stops. Stripe's eight words and
/// Apple's own map onto four of ours here, so a client never learns either
/// dictionary and a third provider is another arm in this function rather than
/// a release of every app.
///
/// `None` is somebody who has never subscribed, which is most of who sees a
/// paywall.
pub fn unpaid(entitlement: Option<&Entitlement>) -> WireUnpaid {
    let Some(entitlement) = entitlement else {
        return WireUnpaid {
            reason: unpaid_reason_name(UnpaidReason::Never).to_string(),
            ran_out: None,
        };
    };

    let reason = match entitlement.provider {
        BillingProvider::Stripe => from_stripe(&entitlement.status),

        // Nothing writes one yet. Until the Apple rail exists, saying the
        // least is the only honest answer: the word in the row is one this
        // function has never mapped.
        BillingProvider::Apple => UnpaidReason::Never,
    };

    WireUnpaid {
        reason: unpaid_reason_name(reason).to_string(),
        ran_out: entitlement.good_until.map(|until| until.as_secs()),
    }
}

/// Stripe's word for a subscription, as the reason a paywall is being drawn.
///
/// Only ever asked about a session already refused, so whatever the word says,
/// the date we hold has passed.
fn from_stripe(status: &str) -> UnpaidReason {
    match StripeStatus::from_name(status) {
        // A renewal that failed, and a first payment that never landed inside
        // Stripe's 23 hours. A payment was attempted and did not go through,
        // which is the whole of what `Declined` claims.
        StripeStatus::PastDue | StripeStatus::Unpaid | StripeStatus::IncompleteExpired => {
            UnpaidReason::Declined
        }

        StripeStatus::Canceled => UnpaidReason::Ended,

        // Started and not paid for: a card form opened and left, or a 3D
        // Secure challenge closed. Stripe holds the invoice open for 23 hours
        // and the same subscription is handed back to the next press of
        // Subscribe, so the way in is the form again — not waiting.
        StripeStatus::Incomplete => UnpaidReason::Unfinished,

        // Stripe believes this person is paying and the date we hold says they
        // are not. That is a webhook we never received, and it may land a
        // minute from now — so this is the one state where saying anything
        // about a cause would be inventing one.
        StripeStatus::Active => {
            tracing::warn!(
                status,
                "a subscription Stripe calls active has a period that has run out — \
                 a missed webhook, and somebody paying is being refused",
            );

            UnpaidReason::Unconfirmed
        }

        // Neither is reachable by configuration: nothing offers a trial and
        // nothing pauses a subscription. Worth a line if one ever appears, and
        // worth the screen that claims nothing about what came before.
        StripeStatus::Trialing | StripeStatus::Paused | StripeStatus::Other(_) => {
            tracing::warn!(
                status,
                "a subscription status nothing in this app can produce",
            );

            UnpaidReason::Never
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fold from Stripe's eight words to our five. It is the only place a
    /// provider's vocabulary is read, so what each one becomes is written down
    /// here rather than inferred from the flow tests that happen to cross it.
    #[test]
    fn stripes_words_become_ours() {
        // A payment was attempted and did not go through. `incomplete_expired`
        // is here and `incomplete` is not: one is a first payment that never
        // landed, the other is one still in flight.
        assert_eq!(from_stripe("past_due"), UnpaidReason::Declined);
        assert_eq!(from_stripe("unpaid"), UnpaidReason::Declined);
        assert_eq!(from_stripe("incomplete_expired"), UnpaidReason::Declined);

        assert_eq!(from_stripe("canceled"), UnpaidReason::Ended);

        // Nothing has been charged and nothing is on its way: the card form
        // was left open, and finishing it is the answer.
        assert_eq!(from_stripe("incomplete"), UnpaidReason::Unfinished);

        // Stripe believes this person is paying and our date says otherwise,
        // which is a webhook that has not arrived. Telling them it ended would
        // be telling a paying customer their subscription is over.
        assert_eq!(from_stripe("active"), UnpaidReason::Unconfirmed);

        // Unreachable by configuration, and a word we have never seen. Both
        // get the screen that claims nothing about what came before, because
        // every other one of the four states a cause.
        assert_eq!(from_stripe("trialing"), UnpaidReason::Never);
        assert_eq!(from_stripe("paused"), UnpaidReason::Never);
        assert_eq!(from_stripe("refunded"), UnpaidReason::Never);
    }

    /// Nobody has ever subscribed, which is most of who sees a paywall.
    #[test]
    fn no_subscription_at_all_claims_nothing() {
        let unpaid = unpaid(None);

        assert_eq!(unpaid.reason(), UnpaidReason::Never);
        assert_eq!(unpaid.ran_out, None);
    }
}
