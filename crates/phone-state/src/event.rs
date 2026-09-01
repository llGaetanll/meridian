//! What happened. See `docs/ARCHITECTURE.md` — events go in, actions come out,
//! and nothing in between touches the outside world.

use crate::EventRuntime;
use crate::EventSource;
use crate::EventUi;

/// Where an event came from.
///
/// Grouping by origin is what keeps a match on `Event` readable as the app
/// grows, and makes a recorded log read as a story rather than a flat list.
///
/// Note what this enum does *not* carry: a timestamp. Time arrives as its own
/// event, [`EventRuntime::Tick`](crate::EventRuntime), rather than stapled to
/// every other one — so no event has to invent a clock reading it does not
/// have, and a log of events is a recording of the session rather than a list
/// of things that happened at whatever time the replay runs at.
#[derive(Debug, uniffi::Enum)]
pub enum Event {
    FromUi { event: EventUi },
    FromSource { event: EventSource },
    FromRuntime { event: EventRuntime },
}
