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
        }
    }
}
