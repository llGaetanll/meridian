//! What to do about it.

use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionRouter;
use crate::ActionSocket;
use crate::ActionSourcePlaid;
use crate::ActionSourceStripe;
use crate::ActionSourceYahoo;
use crate::ActionStore;
use crate::CallId;
use crate::RequestId;

/// Grouped by destination, the same way the events are by origin.
///
/// A call's id rides on the variant that names where it is going, rather than
/// on each of that destination's own variants: everything sent to a source or
/// to the store is one call, and the answer comes back naming it.
#[derive(Debug)]
pub enum Action {
    ToRouter {
        request_id: RequestId,
        action: ActionRouter,
    },
    ToEmail {
        action: ActionEmail,
    },
    ToSourcePlaid {
        call_id: CallId,
        action: ActionSourcePlaid,
    },
    ToSourceStripe {
        call_id: CallId,
        action: ActionSourceStripe,
    },
    ToSourceYahoo {
        call_id: CallId,
        action: ActionSourceYahoo,
    },
    ToSocket {
        action: ActionSocket,
    },
    ToStore {
        call_id: CallId,
        action: ActionStore,
    },
    ToProcess {
        action: ActionProcess,
    },
}
