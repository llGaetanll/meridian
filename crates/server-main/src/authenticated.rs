//! Which authenticated route a request is, once there is somebody to serve it
//! for.

use crate::ConnId;
use crate::EventRouter;
use crate::rate_limit::Route;

/// A route that had to know who was asking, waiting for the answer to that.
/// One per [`EventRouter`] variant carrying a token.
///
/// Not `Copy`: one of these holds a signed transaction, for the same reason
/// [`Pending::WebhookKey`](crate::Pending) holds a whole webhook — it is what
/// the machine cannot get back from the next event.
#[derive(Debug, Clone)]
pub enum Authenticated {
    /// Opening a bank sign-in, with the `Origin` it was asked from — which
    /// decides where Plaid sends it back, once there is somebody to open it
    /// for.
    LinkRequested {
        origin: Option<String>,
    },

    /// Repairing a bank whose login has expired — Plaid's update mode. The
    /// `origin` plays the same part as on
    /// [`LinkRequested`](Authenticated::LinkRequested); the Item is checked
    /// against the caller's banks once they are loaded.
    RepairRequested {
        origin: Option<String>,
        item_id: String,
    },

    LinkClosed,

    /// The stream this request opens, carried through the session read so the
    /// answer knows which connection it is accepting.
    SyncWatchRequested {
        conn_id: ConnId,
    },

    SnapshotRequested,

    /// Starting a subscription. One of the routes an unpaid session may
    /// reach — see [`Authenticated::needs_entitlement`].
    SubscribeRequested,

    /// Opening the Stripe Checkout page that sells the subscription. Another,
    /// for the same reason. The `origin` plays the same part as on
    /// [`LinkRequested`](Authenticated::LinkRequested): it is where the
    /// page's window is sent back.
    CheckoutRequested {
        origin: Option<String>,
    },

    /// Asking what the subscription is. Another.
    BillingStatusRequested,

    /// Saying what to call them. Carries the name, because by the time the
    /// session has resolved the event it came in on is gone.
    NameSubmitted {
        name: String,
    },

    /// Recording a purchase the App Store has already taken. The third route
    /// an unpaid session may reach, and the one that stops it being unpaid.
    ///
    /// Carries what Apple signed, because by the time the session has resolved
    /// the event it came in on is gone.
    AppleTransactionReceived {
        signed: String,
    },

    /// Asking for the code that confirms a deletion, emailed to the address
    /// the account is written to.
    DeleteCodeRequested,

    /// Cancelling and deleting, which are one action — once the code says the
    /// inbox agreed.
    DeleteRequested {
        code: String,
    },

    /// Asking for one symbol's bars over one window.
    BarsRequested {
        symbol: String,
        interval: src_postgres::BarInterval,
        start: api_common::Timestamp,
        end: api_common::Timestamp,
    },
}

impl Authenticated {
    /// Which of these an event is, for the machine to come back to once the
    /// session has resolved. `None` is a route that asks nobody's name.
    pub fn of(event: &EventRouter) -> Option<Self> {
        match event {
            EventRouter::LinkRequested { origin, .. } => Some(Authenticated::LinkRequested {
                origin: origin.clone(),
            }),
            EventRouter::RepairRequested {
                origin, item_id, ..
            } => Some(Authenticated::RepairRequested {
                origin: origin.clone(),
                item_id: item_id.clone(),
            }),
            EventRouter::LinkClosed { .. } => Some(Authenticated::LinkClosed),
            EventRouter::SyncWatchRequested { conn_id, .. } => {
                Some(Authenticated::SyncWatchRequested { conn_id: *conn_id })
            }
            EventRouter::SnapshotRequested { .. } => Some(Authenticated::SnapshotRequested),
            EventRouter::SubscribeRequested { .. } => Some(Authenticated::SubscribeRequested),
            EventRouter::CheckoutRequested { origin, .. } => {
                Some(Authenticated::CheckoutRequested {
                    origin: origin.clone(),
                })
            }
            EventRouter::BillingStatusRequested { .. } => {
                Some(Authenticated::BillingStatusRequested)
            }
            EventRouter::NameSubmitted { name, .. } => {
                Some(Authenticated::NameSubmitted { name: name.clone() })
            }
            EventRouter::AppleTransactionReceived { signed, .. } => {
                Some(Authenticated::AppleTransactionReceived {
                    signed: signed.clone(),
                })
            }
            EventRouter::DeleteCodeRequested { .. } => Some(Authenticated::DeleteCodeRequested),
            EventRouter::DeleteRequested { code, .. } => {
                Some(Authenticated::DeleteRequested { code: code.clone() })
            }
            EventRouter::BarsRequested {
                symbol,
                interval,
                start,
                end,
                ..
            } => Some(Authenticated::BarsRequested {
                symbol: symbol.clone(),
                interval: *interval,
                start: *start,
                end: *end,
            }),
            _ => None,
        }
    }

    /// The bucket a request for this counts against, once it has a caller.
    pub fn route(&self) -> Route {
        match self {
            Authenticated::LinkRequested { .. } => Route::LinkStart,
            Authenticated::RepairRequested { .. } => Route::LinkRepair,
            Authenticated::LinkClosed => Route::LinkFinish,
            Authenticated::SyncWatchRequested { .. } => Route::SyncStatus,
            Authenticated::SnapshotRequested => Route::Snapshot,
            Authenticated::SubscribeRequested => Route::BillingSubscribe,
            Authenticated::CheckoutRequested { .. } => Route::BillingCheckout,
            Authenticated::BillingStatusRequested => Route::BillingStatus,
            Authenticated::NameSubmitted { .. } => Route::AccountName,
            Authenticated::AppleTransactionReceived { .. } => Route::BillingApple,
            Authenticated::DeleteCodeRequested => Route::AccountDeleteRequest,
            Authenticated::DeleteRequested { .. } => Route::AccountDelete,
            Authenticated::BarsRequested { .. } => Route::Bars,
        }
    }

    /// Whether this route is refused without a subscription.
    ///
    /// A method rather than a check at each call site, so adding a route
    /// forces the decision: a new variant does not compile until this match
    /// says which it is. There are no unpaid users, so the answer is `true`
    /// for everything the app actually does.
    ///
    /// The three that answer `false` are the way out of that: a session that
    /// has not paid can start a subscription, hand over one the App Store has
    /// already taken, and ask how it is going. Refusing those would leave
    /// somebody signed in with no route to becoming a customer — and on the
    /// phone, having paid Apple and being refused by us.
    pub fn needs_entitlement(&self) -> bool {
        match self {
            // Deleting is here for a reason worth stating: somebody whose
            // subscription has lapsed must still be able to get their data
            // out of our hands. A deletion refused for want of a payment is a
            // deletion right that does not exist.
            Authenticated::SubscribeRequested
            | Authenticated::CheckoutRequested { .. }
            | Authenticated::BillingStatusRequested
            | Authenticated::AppleTransactionReceived { .. }
            | Authenticated::DeleteCodeRequested
            | Authenticated::DeleteRequested { .. } => false,

            // The name is asked for on the far side of the paywall — first
            // run only reaches that screen once a snapshot has been served —
            // so it is entitled like everything else the app does.
            Authenticated::LinkRequested { .. }
            | Authenticated::RepairRequested { .. }
            | Authenticated::LinkClosed
            | Authenticated::SyncWatchRequested { .. }
            | Authenticated::SnapshotRequested
            | Authenticated::NameSubmitted { .. }
            | Authenticated::BarsRequested { .. } => true,
        }
    }
}
