//! The runtime did something the machine cannot do for itself.
//!
//! One event so far, and it is the clock. Everything else the app knows either
//! came from the user ([`EventUi`](crate::EventUi)) or from something it asked
//! for ([`EventSource`](crate::EventSource)); time is neither, and it is not
//! something the machine may go and read.

#[derive(Debug, uniffi::Enum)]
pub enum EventRuntime {
    /// What time it is now, in seconds since the epoch.
    ///
    /// The clock is an event rather than an argument to `handle_events`, and
    /// that is the whole point: nothing inside the machine reads one, so a
    /// replay of the log lands where it did the first time. The runtime sends
    /// one about once a second.
    ///
    /// A bare `u64` rather than a [`Timestamp`](api_common::Timestamp),
    /// because this crosses FFI and `custom_newtype!` cannot be applied to a
    /// type from another crate — the same reason
    /// [`RangeSelection::Between`](crate::RangeSelection) carries integers.
    /// The conversion happens where the tick is handled, so nothing downstream
    /// sees a raw one.
    Tick { now: u64 },
}
