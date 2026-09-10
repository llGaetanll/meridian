//! Which authenticated route a request is, once there is somebody to serve it
//! for.

use crate::ConnId;
use crate::EventRouter;
use crate::rate_limit::Route;

/// A route that had to know who was asking, waiting for the answer to that.
/// One per [`EventRouter`] variant carrying a token.
#[derive(Debug, Clone, Copy)]
pub enum Authenticated {
    LinkRequested,
    LinkClosed,

    /// The stream this request opens, carried through the session read so the
    /// answer knows which connection it is accepting.
    SyncWatchRequested {
        conn_id: ConnId,
    },

    SnapshotRequested,

    /// Starting a subscription. One of the two routes an unpaid session may
    /// reach — see [`Authenticated::needs_entitlement`].
    SubscribeRequested,

    /// Asking what the subscription is. The other one.
    BillingStatusRequested,

    /// Cancelling and deleting, which are one action.
    DeleteRequested,
}

impl Authenticated {
    /// Which of these an event is, for the machine to come back to once the
    /// session has resolved. `None` is a route that asks nobody's name.
    pub fn of(event: &EventRouter) -> Option<Self> {
        match event {
            EventRouter::LinkRequested { .. } => Some(Authenticated::LinkRequested),
            EventRouter::LinkClosed { .. } => Some(Authenticated::LinkClosed),
            EventRouter::SyncWatchRequested { conn_id, .. } => {
                Some(Authenticated::SyncWatchRequested { conn_id: *conn_id })
            }
            EventRouter::SnapshotRequested { .. } => Some(Authenticated::SnapshotRequested),
            EventRouter::SubscribeRequested { .. } => Some(Authenticated::SubscribeRequested),
            EventRouter::BillingStatusRequested { .. } => {
                Some(Authenticated::BillingStatusRequested)
            }
            EventRouter::DeleteRequested { .. } => Some(Authenticated::DeleteRequested),
            _ => None,
        }
    }

    /// The bucket a request for this counts against, once it has a caller.
    pub fn route(&self) -> Route {
        match self {
            Authenticated::LinkRequested => Route::LinkStart,
            Authenticated::LinkClosed => Route::LinkFinish,
            Authenticated::SyncWatchRequested { .. } => Route::SyncStatus,
            Authenticated::SnapshotRequested => Route::Snapshot,
            Authenticated::SubscribeRequested => Route::BillingSubscribe,
            Authenticated::BillingStatusRequested => Route::BillingStatus,
            Authenticated::DeleteRequested => Route::AccountDelete,
        }
    }

    /// Whether this route is refused without a subscription.
    ///
    /// A method rather than a check at each call site, so adding a route
    /// forces the decision: a new variant does not compile until this match
    /// says which it is. There are no unpaid users, so the answer is `true`
    /// for everything the app actually does.
    ///
    /// The two that answer `false` are the way out of that: a session that
    /// has not paid can start a subscription and ask how it is going, and
    /// nothing else. Refusing those would leave somebody signed in with no
    /// route to becoming a customer.
    pub fn needs_entitlement(&self) -> bool {
        match self {
            // Deleting is here for a reason worth stating: somebody whose
            // subscription has lapsed must still be able to get their data
            // out of our hands. A deletion refused for want of a payment is a
            // deletion right that does not exist.
            Authenticated::SubscribeRequested
            | Authenticated::BillingStatusRequested
            | Authenticated::DeleteRequested => false,

            Authenticated::LinkRequested
            | Authenticated::LinkClosed
            | Authenticated::SyncWatchRequested { .. }
            | Authenticated::SnapshotRequested => true,
        }
    }
}
