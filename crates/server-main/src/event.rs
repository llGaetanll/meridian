//! What happened to the server.

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
    /// The id says which handler is parked on the answer. The machine relays
    /// it and nothing more — see [`RequestId`](crate::RequestId).
    FromRouter {
        request_id: RequestId,
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
