//! A source answered, or failed to.

/// Note that neither variant carries a clock reading. The runtime hands the
/// time to `handle_events` once per batch; a source's job is to say what came
/// back, not when it happened to arrive.
#[derive(Debug, uniffi::Enum)]
pub enum EventSource {
    /// A snapshot arrived. Everything the app knows, in one value.
    ///
    /// The payload is JSON rather than a structured record because the
    /// alternative is redeclaring every wire type as a UniFFI record and
    /// keeping the two in step by hand. The machine parses it, which keeps the
    /// parse in one place for all three platforms.
    SnapshotLoaded { json: String },

    /// A request did not come back.
    LoadFailed { message: String },
}
