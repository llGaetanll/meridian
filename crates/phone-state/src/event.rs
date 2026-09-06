//! What happened. See `docs/ARCHITECTURE.md`.

use crate::EventRuntime;
use crate::EventSource;
use crate::EventUi;

/// Where an event came from.
///
/// No timestamp: time arrives as its own event,
/// [`EventRuntime::Tick`](crate::EventRuntime), so a log of events is a
/// recording of the session rather than of whenever the replay runs.
#[derive(Debug, uniffi::Enum)]
pub enum Event {
    FromUi { event: EventUi },
    FromSource { event: EventSource },
    FromRuntime { event: EventRuntime },
}
