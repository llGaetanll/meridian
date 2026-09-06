//! What happened to the server.

use crate::CallId;
use crate::Client;
use crate::EventProcess;
use crate::EventRouter;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;
use crate::RequestId;

/// Grouped by origin, the same way the app's events are.
#[derive(Debug)]
pub enum Event {
    /// A route was asked for something.
    ///
    /// The id and the client ride here rather than on each
    /// [`EventRouter`](crate::EventRouter) variant, because both are true of
    /// *every* request.
    FromRouter {
        request_id: RequestId,
        client: Client,
        event: EventRouter,
    },

    /// A source answered, or failed to.
    FromSource { event: EventSource },

    /// The store answered, or failed to.
    FromStore { call_id: CallId, event: EventStore },

    /// The process did something to itself.
    FromProcess { event: EventProcess },

    /// The process was signalled.
    FromTerminal { event: EventTerminal },
}
