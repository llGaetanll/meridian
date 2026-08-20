//! What happened to the server.

use crate::EventClient;
use crate::EventSource;
use crate::EventTerminal;

/// Grouped by origin, the same way the app's events are — a match on this stays
/// readable as the server grows, and a recorded log reads as a story.
#[derive(Debug)]
pub enum Event {
    /// A client asked for something.
    FromClient { event: EventClient },

    /// A source answered, or failed to.
    FromSource { event: EventSource },

    /// The process was signalled.
    FromTerminal { event: EventTerminal },
}
