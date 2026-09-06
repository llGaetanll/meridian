//! The runtime did something the machine cannot do for itself.

#[derive(Debug, uniffi::Enum)]
pub enum EventRuntime {
    /// What time it is, as the device's clock reads it — seconds since the
    /// epoch, about once a second.
    ///
    /// A bare `u64` rather than a [`Timestamp`](api_common::Timestamp),
    /// because this crosses FFI and `custom_newtype!` cannot be applied to a
    /// type from another crate.
    Tick { now: u64 },
}
