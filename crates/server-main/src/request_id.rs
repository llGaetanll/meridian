//! Which route is waiting.

/// Names one request in flight, so the answer reaches the handler that asked.
///
/// Minted at the edge and only ever relayed by the machine, which never
/// compares two or decides anything differently because of one — so a replay
/// lands where it did regardless of which numbers were drawn.
pub type RequestId = u64;
