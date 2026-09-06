//! Which question this is the answer to.

/// Names one call out to a source, so its answer can be told from another's.
///
/// Minted by the machine, so the counter is state like any other and the same
/// event log draws the same numbers.
///
/// Not a [`RequestId`](crate::RequestId): a call may have been made for a
/// waiting route, or on nobody's behalf at all — a sync on a timer answers to
/// no one.
pub type CallId = u64;
