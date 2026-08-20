//! What happened. See `docs/ARCHITECTURE.md` — events go in, actions come out,
//! and nothing in between touches the outside world.

use crate::EventSource;
use crate::EventUi;

/// Where an event came from.
///
/// Grouping by origin is what keeps a match on `Event` readable as the app
/// grows, and makes a recorded log read as a story rather than a flat list.
///
/// Note what this enum does *not* carry: a timestamp. Time is handed to
/// [`StateApp::handle_events`](crate::StateApp::handle_events) once per batch
/// rather than stapled to every event, so no event has to invent a clock
/// reading it does not have.
#[derive(Debug, uniffi::Enum)]
pub enum Event {
    FromUi { event: EventUi },
    FromSource { event: EventSource },
}
