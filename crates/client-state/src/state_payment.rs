//! Paying, from the paywall's button to the server hearing of it.
//!
//! A sub-machine for the reason first run is one: nothing outside it reads its
//! steps, and once somebody is served there is nothing left to read.
//!
//! Three parties, and only two of them answer. Our server starts a
//! subscription; the payment provider takes the card; and the provider then
//! tells our server, which nothing here hears except by asking again. So a
//! card that was taken is not yet an app that opens, and the last step is a
//! wait with a limit.

use std::time::Duration;

use api_common::Timestamp;

use crate::Action;
use crate::ActionSource;
use crate::PaymentRefusal;

/// How often to ask whether the server has heard, once the card is taken.
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

    /// A subscription asked for, and not yet started.
    Starting,

    /// Started, and the card form up. `refused` is why the last card was not
    /// taken, where one was tried and was not.
    Card { refused: Option<PaymentRefusal> },

    /// A card handed to the provider, and no answer yet.
    Taking,

    /// Taken, and the server is being asked whether it has heard. `since` is
    /// when the asking began and `asked` the last time it asked.
    Confirming { since: Timestamp, asked: Timestamp },

    /// Taken, and the server still had not heard when the asking stopped.
    /// Asking again is a button.
    Unconfirmed,
}

impl StatePayment {
    /// Subscribe. A subscription is started only from the offer: one that is
    /// started and not paid for is the one a second card pays, and starting
    /// another would leave the first behind at the provider.
    pub(crate) fn subscribe(&mut self, actions: &mut Vec<Action>) {
        if let Self::Offer = self {
            *self = Self::Starting;

            actions.push(Action::ToSource {
                action: ActionSource::StartSubscription,
            });
        }
    }

    /// The server already has a subscription for this person, so there was
    /// nothing to start.
    ///
    /// Skips to asking whether the server has heard of a payment, which is
    /// the step this would have reached anyway had a card been taken. If that
    /// subscription is paid for, the snapshot is served and the app opens; if
    /// it is one whose card stopped working, the paywall comes back saying so.
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

    /// Pay, with the card in the form.
    pub(crate) fn pay(&mut self, actions: &mut Vec<Action>) {
        if let Self::Card { .. } = self {
            *self = Self::Taking;

            actions.push(Action::ToSource {
                action: ActionSource::TakePayment,
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

    pub(crate) fn started(&mut self) {
        match self {
            Self::Starting => *self = Self::Card { refused: None },
            _ => tracing::warn!("a subscription started that nothing was waiting on"),
        }
    }

    pub(crate) fn taken(&mut self, now: Timestamp, actions: &mut Vec<Action>) {
        match self {
            Self::Taking => {
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

    pub(crate) fn refused(&mut self, refusal: PaymentRefusal) {
        match self {
            Self::Taking => {
                *self = Self::Card {
                    refused: Some(refusal),
                }
            }
            _ => tracing::warn!("a payment was refused that nothing was waiting on"),
        }
    }

    /// A call to the server failed. Of the steps here only starting a
    /// subscription waits on the server, so that is the one put back; a
    /// failed ask while confirming is simply asked again.
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
