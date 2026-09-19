//! The paywall, and paying from it, turned into a screen.
//!
//! Every sentence it prints is here, so the web and the desktop app word the
//! same refusal and the same card the same way — and the phone the same
//! refusal, until it sells.

use api_common::Timestamp;
use api_common::format::fmt_date;

use crate::Unpaid;
use crate::ViewIconRow;
use crate::ViewPaywall;
use crate::ViewPaywallStep;
use crate::ViewRowTone;
use crate::ViewRun;
use crate::state_payment::StatePayment;

/// What Meridian costs each time it is charged, as the provider bills it.
///
/// Not on the wire: a refusal says whether somebody has paid, never what it
/// would cost them to.
const AMOUNT: &str = "$5.99";

/// The paywall, worded for what the server said and for how far paying has
/// got.
///
/// Five refusals rather than one, because "you have never subscribed" and
/// "your card stopped working" are different news and only one of them is
/// about a card. The provider's own vocabulary does not reach here: [`Unpaid`]
/// is our words, and mapping a provider's onto them is the server's job.
pub(crate) fn paywall(unpaid: &Unpaid, payment: StatePayment) -> ViewPaywall {
    let (title, body) = match payment {
        StatePayment::Offer | StatePayment::Starting | StatePayment::Paying => refusal(unpaid),

        StatePayment::Confirming { .. } => (
            "Payment received",
            "Meridian opens as soon as Stripe confirms it with us, which usually takes a \
             few seconds.",
        ),

        StatePayment::Unconfirmed => (
            "We can't confirm your payment yet",
            "Your card was charged, and Stripe hasn't told us yet. Nothing has been lost: \
             check again in a minute.",
        ),
    };

    let mut rows = vec![ViewIconRow {
        icon: "tag".to_string(),
        tone: ViewRowTone::Plain,
        title: "Price".to_string(),
        sub: None,
        value: Some(format!("{AMOUNT} a month")),
    }];

    // Always in the past: entitlement is that date against the clock, so a
    // period still running is a session that never reached this screen.
    if let Some(ran_out) = ran_out(unpaid) {
        rows.push(ViewIconRow {
            icon: "clock".to_string(),
            tone: ViewRowTone::Plain,
            title: "Ran out".to_string(),
            sub: None,
            value: Some(fmt_date(ran_out)),
        });
    }

    ViewPaywall {
        title: title.to_string(),
        body: vec![ViewRun {
            text: body.to_string(),
            strong: false,
        }],
        rows,
        step: step(payment),
        sign_out: "Sign out".to_string(),
    }
}

/// What the server's refusal reads as, before anybody has pressed anything.
fn refusal(unpaid: &Unpaid) -> (&'static str, &'static str) {
    match unpaid {
        Unpaid::Never => (
            "Meridian is a subscription",
            "Your account is ready. There is one price and nothing else to choose, and you \
             can leave whenever you like.",
        ),

        Unpaid::Declined { .. } => (
            "Your last payment didn't go through",
            "The card on file was declined. A working card opens the app back up.",
        ),

        Unpaid::Ended { .. } => (
            "Your subscription has ended",
            "It ran to the end of the period you had paid for, and that has now passed. \
             Subscribing again opens the app back up.",
        ),

        Unpaid::Unfinished => (
            "You haven't finished subscribing",
            "A subscription was started and no payment went through, so nothing has \
             been charged. Subscribing again on the web picks up where it left off.",
        ),

        Unpaid::Unconfirmed => (
            "We can't confirm your subscription",
            "If you have just paid, give it a minute and pull to refresh. Nothing here \
             has been lost.",
        ),
    }
}

fn step(payment: StatePayment) -> ViewPaywallStep {
    match payment {
        StatePayment::Offer => ViewPaywallStep::Offer {
            action: "Pay by card".to_string(),
            opening: false,
        },

        StatePayment::Starting => ViewPaywallStep::Offer {
            action: "Opening Stripe…".to_string(),
            opening: true,
        },

        StatePayment::Paying => ViewPaywallStep::Offer {
            action: "Finish on Stripe's page…".to_string(),
            opening: true,
        },

        StatePayment::Confirming { .. } => ViewPaywallStep::Confirming {
            action: "Confirming…".to_string(),
            checking: true,
        },

        StatePayment::Unconfirmed => ViewPaywallStep::Confirming {
            action: "Check again".to_string(),
            checking: false,
        },
    }
}

/// When the period that was paid for ended, where there was one.
fn ran_out(unpaid: &Unpaid) -> Option<Timestamp> {
    match unpaid {
        Unpaid::Never | Unpaid::Unfinished | Unpaid::Unconfirmed => None,
        Unpaid::Declined { ran_out } | Unpaid::Ended { ran_out } => {
            ran_out.map(Timestamp::from_secs)
        }
    }
}
