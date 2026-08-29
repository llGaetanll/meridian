//! What happened to the server.

use crate::EventClient;
use crate::EventProcess;
use crate::EventSource;
use crate::EventStore;
use crate::EventTerminal;

/// Grouped by origin, the same way the app's events are — a match on this stays
/// readable as the server grows, and a recorded log reads as a story.
/// Inherits the size difference from [`EventSource`](crate::EventSource),
/// where the reason is written out.
#[expect(
    clippy::large_enum_variant,
    reason = "a sync is large and has to cross; see EventSource"
)]
#[derive(Debug)]
pub enum Event {
    /// A client asked for something.
    FromClient { event: EventClient },

    /// A source answered, or failed to.
    FromSource { event: EventSource },

    /// The store answered, or failed to.
    FromStore { event: EventStore },

    /// The process did something to itself — so far, started.
    FromProcess { event: EventProcess },

    /// The process was signalled.
    FromTerminal { event: EventTerminal },
}
