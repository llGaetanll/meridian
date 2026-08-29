//! Which question this is the answer to.

/// Names one call out to a source, so its answer can be told from another's.
///
/// Minted by the machine, because the machine is what decided to make the
/// call. That is also what makes it replayable: the counter is state like any
/// other, so the same event log draws the same numbers.
///
/// It is not a [`RequestId`](crate::RequestId). A call may have been made for
/// a route that is waiting, or on nobody's behalf at all — a sync on a timer
/// answers to no one. The machine holds the join between the two, and an
/// absent entry is what "nobody is waiting on this" looks like.
pub type CallId = u64;
