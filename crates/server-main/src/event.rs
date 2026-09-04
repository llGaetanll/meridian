//! What happened to the server.

use crate::Client;
use crate::EventProcess;
use crate::EventRouter;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;
use crate::RequestId;

/// Grouped by origin, the same way the app's events are — a match on this stays
/// readable as the server grows, and a recorded log reads as a story.
#[derive(Debug)]
pub enum Event {
    /// A route was asked for something.
    ///
    /// The id says which handler is parked on the answer, and the client says
    /// who asked. The machine relays the first and buckets by the second — see
    /// [`RequestId`](crate::RequestId) and [`Client`](crate::Client).
    ///
    /// Both ride here rather than on each [`EventRouter`](crate::EventRouter)
    /// variant, because both are true of *every* request and neither is part
    /// of what any one route is asking for. It is also what makes the rate
    /// limit unforgettable: a new route gets one whether its author thought
    /// about it or not.
    FromRouter {
        request_id: RequestId,
        client: Client,
        event: EventRouter,
    },

    /// A source answered, or failed to.
    FromSource { event: EventSource },

    /// The store answered, or failed to.
    FromStore { event: EventStore },

    /// The process did something to itself — so far, started.
    FromProcess { event: EventProcess },

    /// The process was signalled.
    FromTerminal { event: EventTerminal },
}
