//! Which route is waiting.

/// Names one request in flight, so the answer reaches the handler that asked.
///
/// Minted outside the machine, at the edge, and carried in on
/// [`Event::FromRouter`](crate::Event) — the same way the clock and the seed
/// arrive. The machine only ever relays it: it never compares two, and never
/// decides anything differently because of one. A replay of the log therefore
/// lands where it did the first time regardless of which numbers were drawn.
pub type RequestId = u64;
