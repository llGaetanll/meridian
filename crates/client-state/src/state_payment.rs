//! Paying, from the paywall's button to the server hearing of it.
//!
//! A sub-machine for the reason first run is one: nothing outside it reads its
//! steps, and once somebody is served there is nothing left to read.
//!
//! Three parties, and only two of them answer. Our server opens a page of
//! Stripe's; Stripe takes the money on it; and Stripe then tells our server,
//! which nothing here hears except by asking again. So a sheet that took the
//! money is not yet an app that opens, and the last step is a wait with a
//! limit.

use std::time::Duration;

use api_common::Timestamp;

use crate::Action;
use crate::ActionSource;

/// How often to ask whether the server has heard, once the money is taken.
const CONFIRM_EVERY: Duration = Duration::from_secs(3);

/// How long to go on asking on its own before handing the asking to a button.
///
/// The provider's word ordinarily reaches the server in a second or two. A
/// minute without it is a message that is not coming soon, and asking every
/// three seconds for longer is a poll against an answer that will not change.
pub(crate) const CONFIRM_FOR: Duration = Duration::from_secs(60);

/// Where paying is. The variant is the step.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) enum StatePayment {
    /// Nothing started.
    #[default]
    Offer,

    /// Stripe's page asked for, and not yet up.
    Starting,

    /// Stripe's page is up, in a window of its own, and the money is taken
    /// there or not at all. Nothing here hears from the page; the window
    /// going away is the next event.
    Paying,

    /// The sheet took the money, and the server is being asked whether it has
    /// heard. `since` is when the asking began and `asked` the last time it
    /// asked.
    Confirming { since: Timestamp, asked: Timestamp },

    /// Taken, and the server still had not heard when the asking stopped.
    /// Asking again is a button.
    Unconfirmed,
}

impl StatePayment {
    /// Pay by card. Stripe's page is opened only from the offer: while one is
    /// up, the money is being taken there and a second window would race it.
    pub(crate) fn subscribe(&mut self, actions: &mut Vec<Action>) {
        if let Self::Offer = self {
            *self = Self::Starting;

            actions.push(Action::ToSource {
                action: ActionSource::StartCheckout,
            });
        }
    }

    /// The server already has a subscription for this person, so there was no
    /// page to open.
    ///
    /// Skips to asking whether the server has heard of a payment, which is
    /// the step this would have reached anyway had the sheet taken a card. If
    /// that subscription is paid for, the snapshot is served and the app
    /// opens; if it is one whose card stopped working, the paywall comes back
    /// saying so.
    pub(crate) fn already(&mut self, now: Timestamp, actions: &mut Vec<Action>) {
        if let Self::Starting = self {
            *self = Self::Confirming {
                since: now,
                asked: now,
            };

            actions.push(Action::ToSource {
                action: ActionSource::LoadSnapshot,
            });
        }
    }

    /// Check again, once the asking has stopped: another minute of it.
    ///
    /// Asks nothing itself. The button is a refresh, and a refresh already
    /// asks.
    pub(crate) fn check_again(&mut self, now: Timestamp) {
        if let Self::Unconfirmed = self {
            *self = Self::Confirming {
                since: now,
                asked: now,
            };
        }
    }

    pub(crate) fn opened(&mut self) {
        match self {
            Self::Starting => *self = Self::Paying,
            _ => tracing::warn!("stripe's page opened and nothing was waiting on it"),
        }
    }

    pub(crate) fn taken(&mut self, now: Timestamp, actions: &mut Vec<Action>) {
        match self {
            Self::Starting | Self::Paying => {
                *self = Self::Confirming {
                    since: now,
                    asked: now,
                };

                actions.push(Action::ToSource {
                    action: ActionSource::LoadSnapshot,
                });
            }
            _ => tracing::warn!("a payment was taken that nothing was waiting on"),
        }
    }

    /// The window went away without the money being taken — closed by hand,
    /// or Stripe's back arrow. The offer again, ready for another press.
    pub(crate) fn closed(&mut self) {
        match self {
            Self::Starting | Self::Paying => *self = Self::Offer,
            _ => tracing::warn!("stripe's page closed and nothing was waiting on it"),
        }
    }

    /// A call to the server failed. Of the steps here only opening Stripe's
    /// page waits on the server, so that is the one put back; a failed ask
    /// while confirming is simply asked again.
    pub(crate) fn failed(&mut self) {
        if let Self::Starting = self {
            *self = Self::Offer;
        }
    }

    /// Ask again whether the server has heard, while it is worth asking.
    ///
    /// Answers whether the step changed, which it does once: on the tick that
    /// the asking stops.
    pub(crate) fn tick(&mut self, now: Timestamp, actions: &mut Vec<Action>) -> bool {
        let Self::Confirming { since, asked } = self else {
            return false;
        };

        if now.since(*since) >= CONFIRM_FOR {
            *self = Self::Unconfirmed;

            return true;
        }

        if now.since(*asked) >= CONFIRM_EVERY {
            *asked = now;

            actions.push(Action::ToSource {
                action: ActionSource::LoadSnapshot,
            });
        }

        false
    }
}
